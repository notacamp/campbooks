# frozen_string_literal: true

module Api
  module App
    # Serializes one FeedItem (a deck card). Subject is kept shallow in the list
    # (kind + id + title) so the list request stays cheap. The full subject is
    # available via the respective /api/app resource endpoints.
    class FeedItemSerializer
      def initialize(item, subject: nil)
        @item    = item
        @subject = subject
      end

      def as_json
        {
          id:           @item.id,
          kind:         @item.kind,
          score:        @item.score.to_f.round(4),
          attention:    @item.attention,
          sort_at:      @item.sort_at&.iso8601,
          subject_type: @item.subject_type,
          subject_id:   @item.subject_id,
          subject:      subject_summary,
          snoozed_until: @item.snoozed_until&.iso8601,
          seen_at:      @item.seen_at&.iso8601,
          dismissed_at: @item.dismissed_at&.iso8601,
          acted_at:     @item.acted_at&.iso8601,
          active:       @item.active?,
          data:         @item.data
        }
      end

      private

      def subject_summary
        return nil unless @subject

        case @subject
        when EmailMessage
          { id: @subject.id, title: @subject.subject, from: @subject.from_address,
            received_at: @subject.received_at&.iso8601 }
        when Task
          { id: @subject.id, title: @subject.title }
        when Reminder
          { id: @subject.id, title: @subject.title, due_at: @subject.due_at&.iso8601 }
        when CalendarEvent
          { id: @subject.id, title: @subject.title, start_at: @subject.start_at&.iso8601 }
        when Document
          { id: @subject.id, title: @subject.display_name }
        else
          { id: @subject.id }
        end
      rescue StandardError
        nil
      end
    end
  end
end
