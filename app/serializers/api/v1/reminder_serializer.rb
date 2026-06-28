# frozen_string_literal: true

module Api
  module V1
    # Serializes a Reminder for the public API. List responses include core fields;
    # pass detail: true (show / confirm / dismiss / snooze) to add justification
    # and extracted_data.
    class ReminderSerializer
      def initialize(reminder, detail: false)
        @reminder = reminder
        @detail = detail
      end

      def as_json
        data = {
          id:               @reminder.id,
          title:            @reminder.title,
          description:      @reminder.description,
          due_at:           @reminder.due_at&.iso8601,
          all_day:          @reminder.all_day,
          reminder_type:    @reminder.reminder_type,
          status:           @reminder.status,
          confidence:       @reminder.confidence,
          amount_cents:     @reminder.amount_cents,
          currency:         @reminder.currency,
          snoozed_until:    @reminder.snoozed_until&.iso8601,
          source_type:      @reminder.source_type,
          source_id:        @reminder.source_id,
          calendar_event_id: @reminder.calendar_event_id,
          created_at:       @reminder.created_at.iso8601
        }

        if @detail
          data[:justification]  = @reminder.justification
          data[:extracted_data] = @reminder.extracted_data
        end

        data
      end
    end
  end
end
