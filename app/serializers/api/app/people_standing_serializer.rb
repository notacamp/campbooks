# frozen_string_literal: true

module Api
  module App
    # Serializes one PeopleStanding row as a directory entry. All data comes
    # from the materialized row — no extra queries at list time. Stand line text
    # is computed from the stored standing result at render time (locale-aware via
    # Current.user set by the auth layer).
    class PeopleStandingSerializer
      def initialize(row)
        @row = row
      end

      def as_json
        standing = @row.standing
        {
          id:               @row.counterpart_id,
          counterpart_type: @row.counterpart_type,
          name:             @row.name,
          subtitle:         @row.subtitle,
          avatar_initial:   @row.avatar_initial,
          avatar_email:     @row.avatar_email,
          needs_you:        @row.needs_you,
          verb:             @row.verb,
          stand_line:       standing_line(standing),
          wait_days:        @row.wait_days,
          unread:           @row.data&.dig("unread") || false,
          score:            @row.score.to_f.round(4),
          last_activity_at: @row.last_activity_at&.iso8601,
          email_message_id: @row.email_message_id,
          feed_item_id:     @row.feed_item_id,
          standing_kind:    @row.standing_kind
        }
      end

      private

      def standing_line(result)
        ::People::StandCopy.line(result)
      rescue StandardError
        nil
      end
    end
  end
end
