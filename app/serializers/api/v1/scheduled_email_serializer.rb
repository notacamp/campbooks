# frozen_string_literal: true

module Api
  module V1
    # Serializes a ScheduledEmail for the public API. List responses omit the
    # body; pass detail: true (show / create / update) to add it.
    class ScheduledEmailSerializer
      def initialize(scheduled_email, detail: false)
        @scheduled_email = scheduled_email
        @detail = detail
      end

      def as_json
        data = {
          id: @scheduled_email.id,
          to: @scheduled_email.to_address,
          cc: @scheduled_email.cc_address,
          bcc: @scheduled_email.bcc_address,
          subject: @scheduled_email.subject,
          status: @scheduled_email.status,
          recurring: @scheduled_email.recurring?,
          rrule: @scheduled_email.rrule,
          scheduled_at: @scheduled_email.scheduled_at&.iso8601,
          next_occurrence_at: @scheduled_email.next_occurrence_at&.iso8601,
          last_sent_at: @scheduled_email.last_sent_at&.iso8601,
          account_id: @scheduled_email.email_account_id,
          created_at: @scheduled_email.created_at.iso8601
        }

        data[:body] = @scheduled_email.body if @detail

        data
      end
    end
  end
end
