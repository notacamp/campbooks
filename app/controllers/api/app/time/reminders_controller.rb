# frozen_string_literal: true

module Api
  module App
    module Time
      # Reminder mutations from the Time surface: confirm (→ calendar event), dismiss,
      # snooze. Mirrors RemindersController but returns JSON.
      class RemindersController < Api::App::BaseController
        before_action :set_reminder

        # POST /api/app/reminders/:id/confirm
        def confirm
          apply_date_edit
          result = Reminders::Confirm.call(@reminder, user: current_user)
          if result.success?
            render_data(reminder_response(@reminder))
          else
            render_error("confirm_failed", result.error, status: :unprocessable_entity)
          end
        end

        # DELETE /api/app/reminders/:id
        def dismiss
          @reminder.dismissed!
          Events.publish("reminder.dismissed", subject: @reminder,
                         payload: { "title" => @reminder.title, "due_at" => @reminder.due_at&.iso8601 })
          render_data(reminder_response(@reminder))
        end

        # POST /api/app/reminders/:id/snooze
        def snooze
          @reminder.update!(status: :snoozed, snoozed_until: snooze_until)
          render_data(reminder_response(@reminder))
        end

        private

        def set_reminder
          @reminder = Reminder.accessible_to(current_user).find(params[:id])
        end

        def apply_date_edit
          return if params[:due_at].blank?

          parsed = ::Time.zone.parse(params[:due_at].to_s)
          @reminder.update!(due_at: parsed) if parsed
        rescue ArgumentError
          nil
        end

        def snooze_until
          return 1.week.from_now if params[:until].blank?

          ::Time.zone.parse(params[:until].to_s) || 1.week.from_now
        rescue ArgumentError
          1.week.from_now
        end

        def reminder_response(reminder)
          {
            item:       Api::V1::ReminderSerializer.new(reminder, detail: true).as_json,
            undo_token: nil
          }
        end
      end
    end
  end
end
