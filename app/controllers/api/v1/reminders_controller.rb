# frozen_string_literal: true

module Api
  module V1
    # Public API for AI-extracted reminders. Reminders are created only by the AI
    # pipeline; the API exposes read access and the three lifecycle transitions
    # (confirm / dismiss / snooze) that mirror the web UI.
    class RemindersController < BaseController
      before_action -> { doorkeeper_authorize! :"reminders:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"reminders:write" }, only: [ :confirm, :dismiss, :snooze ]
      before_action :set_reminder, only: [ :show, :confirm, :dismiss, :snooze ]

      def index
        scope = Reminder.accessible_to(Current.user)
        scope = scope.where(status: params[:status]) if params[:status].present? && Reminder.statuses.key?(params[:status])
        @pagy, records = pagy(scope.order(due_at: :asc), limit: per_page)
        render_page(records.map { |r| ReminderSerializer.new(r).as_json }, @pagy)
      end

      def show
        render_data(ReminderSerializer.new(@reminder, detail: true).as_json)
      end

      def confirm
        if params[:due_at].present?
          parsed = parse_time(params[:due_at])
          unless parsed
            render_api_error("invalid_due_at", "The due_at value could not be parsed.",
                             status: :unprocessable_entity)
            return
          end
          @reminder.update!(due_at: parsed)
        end

        result = Reminders::Confirm.call(@reminder, user: Current.user)
        if result.success?
          serialized = ReminderSerializer.new(@reminder.reload, detail: true).as_json
          render_data(serialized.merge(calendar_event_id: result.calendar_event&.id))
        else
          render_api_error("confirm_failed", result.error || "Could not confirm.",
                           status: :unprocessable_entity)
        end
      end

      def dismiss
        @reminder.dismissed!
        render_data(ReminderSerializer.new(@reminder, detail: true).as_json)
      end

      def snooze
        @reminder.update!(status: :snoozed, snoozed_until: snooze_until)
        render_data(ReminderSerializer.new(@reminder, detail: true).as_json)
      end

      private

      def set_reminder
        @reminder = Reminder.accessible_to(Current.user).find(params[:id])
      end

      def snooze_until
        return 1.week.from_now if params[:until].blank?
        Time.zone.parse(params[:until]) || 1.week.from_now
      rescue ArgumentError
        1.week.from_now
      end

      def parse_time(value)
        Time.zone.parse(value.to_s)
      rescue ArgumentError
        nil
      end
    end
  end
end
