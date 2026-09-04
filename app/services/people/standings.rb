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

        directory  = People::Directory.new(user, workspace: workspace, now: now)
        counterparts = directory.counterparts

        ApplicationRecord.transaction do
          if counterparts.any?
            rows = counterparts.map { |cp| build_row(cp, user: user, workspace: workspace, now: now, directory: directory) }
            PeopleStanding.upsert_all(
              rows,
              unique_by: :index_people_standings_on_user_counterpart,
              update_only: %i[
                needs_you standing_kind text email_thread_id overdue_days
                score strength last_activity_at name subtitle avatar_email
                avatar_initial data verb subject wait_days feed_item_id
                email_message_id refreshed_at
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

        counterparts.size
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
          email_message_id:  nil, # reserved for PR 3 row actions
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
    end
  end
end
