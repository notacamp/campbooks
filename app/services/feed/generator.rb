module Feed
  # Populates `feed_items` for one user from every Feed::Source.
  #
  #   1. Collect candidates from each source (in registry order).
  #   2. Dedup by subject — the first source to claim a record wins, so an aged
  #      email surfaces as a reminder rather than also as an action.
  #   3. upsert_all the survivors, preserving per-user state (seen/dismissed/acted)
  #      and stamping generated_at = run time.
  #   4. Reconcile: any active row a successful source *didn't* refresh this run
  #      (older generated_at) no longer qualifies — resolve it.
  #
  # Idempotent: running it twice with no underlying change is a no-op (the unique
  # (user_id, dedupe_key) index turns inserts into updates).
  class Generator
    def self.for_user(user, now: Time.current)
      new(user, now: now).call
    end

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    def call
      return 0 unless @user&.workspace_id

      rows = []
      ran_kinds = []
      claimed = {}

      Feed::Source.all.each do |klass|
        source = klass.new(@user, now: @now)
        candidates =
          begin
            source.candidates
          rescue => e
            # A failing source must not let reconcile wipe its existing items, so
            # skip both its candidates and its kind from this run.
            Rails.logger.error("[Feed::Generator] #{klass.name} failed: #{e.class}: #{e.message}")
            next
          end

        ran_kinds << klass.key
        candidates.each do |c|
          subject = c[:subject]
          next if claimed[claim_key(subject)] # cross-source dedup: first source wins
          claimed[claim_key(subject)] = true
          rows << row_for(klass.key, c, subject)
        end
      end

      persist(rows, ran_kinds)
      rows.size
    end

    private

    # One card per email *thread* (not per message) across all sources, so an
    # aged reminder and a fresh action on the same thread can't both surface.
    # Non-threaded subjects (loose emails, documents) dedupe by record.
    def claim_key(subject)
      if subject.is_a?(EmailMessage) && subject.email_thread_id
        "EmailThread:#{subject.email_thread_id}"
      else
        "#{subject.class.polymorphic_name}:#{subject.id}"
      end
    end

    def row_for(kind, candidate, subject)
      {
        user_id: @user.id,
        workspace_id: @user.workspace_id,
        kind: kind,
        subject_type: subject.class.polymorphic_name,
        subject_id: subject.id,
        dedupe_key: candidate[:dedupe_key],
        sort_at: candidate[:sort_at],
        score: candidate[:score] || 0,
        attention: !!candidate[:attention],
        data: candidate[:data] || {},
        generated_at: @now
        # created_at/updated_at are managed automatically by upsert_all.
      }
    end

    def persist(rows, ran_kinds)
      FeedItem.transaction do
        if rows.any?
          # update_only preserves per-user state (seen/dismissed/acted) and
          # created_at on conflict; updated_at is bumped automatically (so it
          # must NOT be listed here — that would assign it twice).
          FeedItem.upsert_all(
            rows,
            unique_by: %i[user_id dedupe_key],
            update_only: %i[sort_at score attention data generated_at]
          )
        end
        reconcile(ran_kinds)
      end
    end

    # Watermark reconcile: produced rows all carry generated_at = @now, so any
    # active row with an older stamp wasn't refreshed and is resolved. Scoped to
    # kinds whose source ran (a crashed source leaves its items untouched).
    def reconcile(ran_kinds)
      return if ran_kinds.empty?

      @user.feed_items.active
           .where(kind: ran_kinds)
           .where("generated_at IS NULL OR generated_at < ?", @now)
           .update_all(acted_at: @now, updated_at: @now)
    end
  end
end
