module Feed
  # Populates `feed_items` for one user from every Feed::Source.
  #
  #   1. Collect candidates from each source (in registry order).
  #   2. Dedup by subject — the first source to claim a record wins, so an aged
  #      email surfaces as a reminder rather than also as an action.
  #   3. Rank: Feed::Ranking turns each source's intrinsic score into the final
  #      one (relevance boosts × recency decay × engagement) and gates the
  #      attention flag. Candidates that rank below MIN_SCORE are dropped —
  #      a fully decayed fossil leaves the feed (Rewind still has it).
  #   4. upsert_all the survivors, preserving per-user state (seen/dismissed/acted),
  #      stamping generated_at = run time and clearing expired_at — a record that
  #      qualifies again after a system-expiry gets its card back.
  #   5. Reconcile: any active row a successful source *didn't* refresh this run
  #      (older generated_at) no longer qualifies — expire it. Expiry is the
  #      system's verdict (expired_at), distinct from the user's (acted/dismissed).
  #
  # Idempotent: running it twice with no underlying change is a no-op (the unique
  # (user_id, dedupe_key) index turns inserts into updates).
  class Generator
    # Final scores below this leave the feed entirely rather than render as
    # fossils at the timeline's bottom forever.
    MIN_SCORE = 8
    def self.for_user(user, now: Time.current)
      new(user, now: now).call
    end

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    def call
      return 0 unless @user&.workspace_id

      picked = []
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
          keys = claim_keys(subject)
          if (idx = keys.filter_map { |k| claimed[k] }.first)
            # Claimed already. Across sources the first source wins (priority
            # order). Within one source, a fragment of the same conversation
            # that sorts newer replaces the earlier pick, so the surviving card
            # is the conversation's latest message — sources iterate in id
            # order, not recency (find_each), so order can't be trusted for this.
            kind, existing = picked[idx]
            if kind == klass.key && c[:sort_at] && existing[:sort_at] && c[:sort_at] > existing[:sort_at]
              picked[idx] = [ klass.key, c ]
              keys.each { |k| claimed[k] = idx }
            end
            next
          end
          idx = picked.size
          keys.each { |k| claimed[k] = idx }
          picked << [ klass.key, c ]
        end
      end

      enrich_seen_state(picked)
      Feed::Ranking.new(@user, now: @now).apply!(picked)
      picked.reject! { |_kind, c| c[:score].to_i < MIN_SCORE }
      rows = picked.map { |kind, c| row_for(kind, c, c[:subject]) }
      persist(rows, ran_kinds)
      rows.size
    end

    private

    # Stamp each candidate with the seen_at of the row it already has (if any),
    # so Feed::Ranking can demote cards the user has been shown for days and
    # not touched. One pluck on the (user_id, dedupe_key) unique index.
    def enrich_seen_state(picked)
      keys = picked.map { |_kind, c| c[:dedupe_key] }
      return if keys.empty?

      seen = @user.feed_items.where(dedupe_key: keys).pluck(:dedupe_key, :seen_at).to_h
      picked.each { |_kind, c| c[:seen_at] = seen[c[:dedupe_key]] }
    end

    # One card per email *conversation* (not per message) across all sources, so
    # an aged reminder and a fresh action on the same thread can't both surface.
    # Non-threaded subjects (loose emails, documents) dedupe by record.
    #
    # Beyond the thread id, an email also claims a per-sender conversation key:
    # encoding-mangled reply subjects and dropped reply headers fragment one
    # conversation into several EmailThread rows, and without the wider key each
    # fragment surfaces its own card (the claim loop keeps the newest fragment).
    def claim_keys(subject)
      return [ "#{subject.class.polymorphic_name}:#{subject.id}" ] unless subject.is_a?(EmailMessage)

      keys = []
      keys << "EmailThread:#{subject.email_thread_id}" if subject.email_thread_id
      keys << conversation_claim(subject)
      keys.compact!
      keys.presence || [ "#{subject.class.polymorphic_name}:#{subject.id}" ]
    end

    # The bare address inside a From field ("Paulo <p@x.pt>" → "p@x.pt").
    ADDRESS_IN_FROM = /[^\s<>"',;]+@[^\s<>"',;]+/

    # nil when there's no usable signal (unknown sender or blank subject) — a
    # blank key must never lump unrelated mail together. Defensive on attribute
    # presence: sources SELECT narrow column lists, and a column missing here
    # must degrade to thread-only claiming, never crash the generation run.
    def conversation_claim(m)
      return nil unless m.has_attribute?(:subject)

      who   = (m.contact_id if m.has_attribute?(:contact_id))
      who ||= (m.from_address.to_s[ADDRESS_IN_FROM]&.downcase if m.has_attribute?(:from_address))
      return nil if who.blank?

      subj = Emails::SubjectNormalizer.conversation_key(m.subject)
      return nil if subj.blank?

      "EmailConv:#{who}:#{subj}"
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
        generated_at: @now,
        # Re-qualifying clears a prior system-expiry (update_only copies this
        # NULL on conflict); user verdicts (acted/dismissed) are never touched.
        expired_at: nil
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
            update_only: %i[sort_at score attention data generated_at expired_at]
          )
        end
        reconcile(ran_kinds)
      end
    end

    # Watermark reconcile: produced rows all carry generated_at = @now, so any
    # active row with an older stamp wasn't refreshed and is expired. Scoped to
    # kinds whose source ran (a crashed source leaves its items untouched).
    # expired_at — not acted_at — so the upsert can revive the row if the
    # record ever qualifies again, and user-action stats stay clean.
    def reconcile(ran_kinds)
      return if ran_kinds.empty?

      @user.feed_items.active
           .where(kind: ran_kinds)
           .where("generated_at IS NULL OR generated_at < ?", @now)
           .update_all(expired_at: @now, updated_at: @now)
    end
  end
end
