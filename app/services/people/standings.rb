# frozen_string_literal: true

module People
  # Materializes the People directory into the `people_standings` table for a
  # given user. One row per counterpart (person or organization) carrying the
  # display bits, Scout's standing, and the priority score — so the People list
  # reads a paginated table instead of recomputing standings on every request.
  #
  # Mirror of Feed::Generator / Feed::RefreshJob: the directory computation runs
  # in People::StandingsRefreshJob and the result is upserted; the request path
  # reads the table and only enqueues a background refresh when the rows are stale.
  module Standings
    class << self
      # Rebuild the standings table for one user. Idempotent: running twice
      # changes nothing except advancing refreshed_at. Runs in a transaction.
      # Returns the number of counterpart rows written.
      def refresh!(user, now: Time.current)
        workspace = user.workspace
        return 0 unless workspace

        # Snapshot the rendered state before the upsert so only real changes broadcast.
        before = fingerprints(user)

        directory  = People::Directory.new(user, workspace: workspace, now: now)
        counterparts = directory.counterparts

        ApplicationRecord.transaction do
          if counterparts.any?
            rows = counterparts.map { |cp| build_row(cp, user: user, workspace: workspace, now: now, directory: directory) }
            # record_timestamps: false prevents Rails from injecting a second
            # `updated_at = CURRENT_TIMESTAMP` that would conflict with our
            # explicit updated_at in update_only (and bypass travel_to in tests).
            PeopleStanding.upsert_all(
              rows,
              unique_by: :index_people_standings_on_user_counterpart,
              record_timestamps: false,
              update_only: %i[
                needs_you standing_kind text email_thread_id overdue_days
                score strength last_activity_at name subtitle avatar_email
                avatar_initial data verb subject wait_days feed_item_id
                email_message_id refreshed_at updated_at
              ]
            )
          end

          # Delete rows for counterparts that are no longer eligible.
          prune_stale!(user, counterparts)
        end

        # Write the streams count into the cache so the request path reads it
        # without running Emails::TagGroups on every page load.
        streams_count = Emails::TagGroups.new(workspace, directory.readable_account_ids)
                                         .build_groups(directory.inbox_folder_ids).size
        Rails.cache.write("people_streams_count_#{user.id}", streams_count, expires_in: 1.hour)

        # Broadcast changed rows. After is reloaded so updated_at reflects the upsert.
        broadcast_changes!(user, before)

        counterparts.size
      end

      # Recompute a single counterpart row for the user and upsert it, or delete it
      # when the person is no longer eligible. Called by People::ActionsController
      # after each action so the list re-renders with current state without running
      # the whole directory. Broadcasts the change to the people_<user_id> stream.
      def refresh_counterpart!(user, counterpart)
        return unless counterpart
        return unless user.workspace

        workspace  = user.workspace
        now        = Time.current
        directory  = People::Directory.new(user, workspace: workspace, now: now)

        cp = if counterpart.is_a?(Person)
               directory.counterpart_for(counterpart)
        else
               # Organizations: recompute via the full directory for now (rare path).
               directory.counterparts.find { |c| c.id == counterpart.id && c.organization? }
        end

        existing_row = PeopleStanding.for_user(user).find_by(counterpart: counterpart)
        before_print = existing_row && fingerprint(existing_row)

        if cp.nil?
          # No longer eligible — delete the row.
          if existing_row
            existing_row.destroy
            broadcast_remove!(user, counterpart)
          end
          return
        end

        row_data = build_row(cp, user: user, workspace: workspace, now: now, directory: directory)
        # record_timestamps: false lets us control updated_at explicitly via update_only,
        # preventing Rails from injecting a second `updated_at = CURRENT_TIMESTAMP` that
        # would (a) collide with our update_only entry and (b) bypass travel_to in tests.
        PeopleStanding.upsert(
          row_data,
          unique_by: :index_people_standings_on_user_counterpart,
          record_timestamps: false,
          update_only: %i[
            needs_you standing_kind text email_thread_id overdue_days
            score strength last_activity_at name subtitle avatar_email
            avatar_initial data verb subject wait_days feed_item_id
            email_message_id refreshed_at updated_at
          ]
        )

        reloaded = PeopleStanding.for_user(user).find_by(counterpart: counterpart)
        return unless reloaded

        # Broadcast if something actually changed.
        broadcast_replace_row!(user, reloaded) if before_print.nil? || fingerprint(reloaded) != before_print
      rescue => e
        Rails.logger.warn("[People::Standings] refresh_counterpart! failed: #{e.class}: #{e.message}")
      end

      # True when the user has no standing rows yet (first visit / new deploy).
      def missing?(user)
        !PeopleStanding.for_user(user).exists?
      end

      # True when all standing rows are older than the given threshold.
      def stale?(user, threshold: 10.minutes)
        return true if missing?(user)

        !PeopleStanding.for_user(user).where("refreshed_at > ?", threshold.ago).exists?
      end

      private

      def build_row(cp, user:, workspace:, now:, directory:)
        standing = cp.standing
        {
          workspace_id:      workspace.id,
          user_id:           user.id,
          counterpart_type:  cp.person? ? "Person" : "Organization",
          counterpart_id:    cp.id,
          needs_you:         cp.needs_you?,
          standing_kind:     standing.kind.to_s,
          text:              standing.text,
          email_thread_id:   standing.thread_id,
          overdue_days:      standing.overdue_days,
          verb:              standing.verb&.to_s,
          subject:           standing.subject,
          wait_days:         standing.wait_days.to_i,
          feed_item_id:      standing.feed_item_id,
          email_message_id:  standing.email_message_id,
          score:             cp.score&.value || 0.0,
          strength:          cp.score&.strength || 0.0,
          last_activity_at:  cp.last_activity,
          name:              cp.name,
          subtitle:          cp.subtitle,
          avatar_email:      cp.avatar_email,
          avatar_initial:    cp.avatar_initial,
          data:              build_data(cp, directory),
          refreshed_at:      now,
          updated_at:        now,
          created_at:        now
        }
      end

      def build_data(cp, directory)
        base = cp.data || {}
        if cp.organization?
          directory.org_row_data(cp.id, extra: base.except("people_count", "services_count"))
        else
          base
        end
      end

      def prune_stale!(user, counterparts)
        return PeopleStanding.for_user(user).delete_all if counterparts.empty?

        existing = PeopleStanding.for_user(user).pluck(:counterpart_type, :counterpart_id, :id)
        fresh_set = counterparts.map { |cp| [ cp.person? ? "Person" : "Organization", cp.id ] }.to_set
        stale_ids = existing.filter_map { |type, cid, id| id unless fresh_set.include?([ type, cid ]) }
        PeopleStanding.where(id: stale_ids).delete_all if stale_ids.any?
      end

      # ── Live broadcasting ─────────────────────────────────────────────────────

      # After a full refresh!, broadcast rows whose updated_at changed and issue
      # a pill for rows that are new (not previously in `before`). Rows that
      # disappeared are broadcast as removes.
      def broadcast_changes!(user, before)
        after_rows = PeopleStanding.for_user(user).to_a
        after = after_rows.to_h { |r| [ r.counterpart_id, fingerprint(r) ] }
        after_set = after.keys.to_set
        before_set = before.keys.to_set

        new_ids   = after_set - before_set
        gone_ids  = before_set - after_set
        changed   = after_rows.select { |r| before.key?(r.counterpart_id) && before[r.counterpart_id] != after[r.counterpart_id] }

        # Replace changed rows in place; past a handful, one pill beats a storm of replaces.
        if changed.size > MAX_ROW_BROADCASTS
          new_ids |= changed.map(&:counterpart_id)
        else
          changed.each { |row| broadcast_replace_row!(user, row) }
        end

        # Remove gone rows.
        gone_ids.each do |cid|
          # We only know the id — remove by DOM id pattern is best-effort.
          Turbo::StreamsChannel.broadcast_remove_to(
            "people_#{user.id}",
            target: "people_row_#{cid}"
          )
        end

        # New rows: show the pill.
        if new_ids.any?
          count = new_ids.size
          Turbo::StreamsChannel.broadcast_replace_to(
            "people_#{user.id}",
            target: "people_new_pill",
            html: new_pill_html(user, count)
          )
        end
      rescue => e
        Rails.logger.warn("[People::Standings] broadcast_changes! failed: #{e.class}: #{e.message}")
      end

      MAX_ROW_BROADCASTS = 25

      def fingerprints(user)
        PeopleStanding.for_user(user).to_h { |r| [ r.counterpart_id, fingerprint(r) ] }
      end

      # What the row renders: any difference here is worth a replace, nothing else is.
      def fingerprint(row)
        [ row.needs_you, row.verb, row.subject, row.wait_days, row.text, row.score.round(2),
          row.name, row.subtitle, row.avatar_email, row.data ]
      end

      def broadcast_replace_row!(user, standing_row)
        I18n.with_locale(user.locale || I18n.default_locale) do
          cp = standing_row.to_counterpart
          html = ApplicationController.render(
            Campbooks::People::CounterpartRow.new(counterpart: cp),
            layout: false
          )
          Turbo::StreamsChannel.broadcast_replace_to(
            "people_#{user.id}",
            target: "people_row_#{standing_row.counterpart_id}",
            html: html
          )
        end
      rescue => e
        Rails.logger.warn("[People::Standings] broadcast_replace_row! failed: #{e.class}: #{e.message}")
      end

      def broadcast_remove!(user, counterpart)
        Turbo::StreamsChannel.broadcast_remove_to(
          "people_#{user.id}",
          target: "people_row_#{counterpart.id}"
        )
      rescue => e
        Rails.logger.warn("[People::Standings] broadcast_remove! failed: #{e.class}: #{e.message}")
      end

      def new_pill_html(user, count)
        I18n.with_locale(user.locale || I18n.default_locale) do
          label = I18n.t("people.index.new_rows", count: count)
          %(<div id="people_new_pill" class="mx-4 mb-1">
              <a href="#{Rails.application.routes.url_helpers.people_path}"
                 data-turbo-frame="people_results"
                 class="inline-flex items-center gap-1.5 rounded-full bg-ember/10 px-3 py-1 text-[12px] font-semibold text-ember hover:bg-ember/20 no-underline">
                #{CGI.escapeHTML(label)}
              </a>
            </div>)
        end
      end
    end
  end
end
