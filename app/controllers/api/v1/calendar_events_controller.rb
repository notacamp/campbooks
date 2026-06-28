# frozen_string_literal: true

module Api
  module V1
    # Public API for calendar events. Reads are scoped to events the acting user
    # may see via CalendarEvent.accessible_to; writes additionally require that
    # the target calendar is writable by the user and enabled for sync.
    class CalendarEventsController < BaseController
      before_action -> { doorkeeper_authorize! :"calendar:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"calendar:write" }, only: [ :create, :update, :destroy, :rsvp ]
      before_action :set_event,              only: [ :show, :update, :destroy, :rsvp ]
      before_action :require_writable_event!, only: [ :update, :destroy, :rsvp ]

      def index
        scope = CalendarEvent.accessible_to(Current.user).order(start_at: :asc)
        start_after  = parse_time(params[:start_after])
        start_before = parse_time(params[:start_before])
        scope = scope.where("start_at >= ?", start_after)  if start_after
        scope = scope.where("start_at <= ?", start_before) if start_before
        scope = scope.where(calendar_id: params[:calendar_id]) if params[:calendar_id].present?
        @pagy, records = pagy(scope, limit: per_page)
        render_page(records.map { |e| CalendarEventSerializer.new(e).as_json }, @pagy)
      end

      def show
        render_data(CalendarEventSerializer.new(@event, detail: true).as_json)
      end

      def create
        calendar = Calendar.where(
          calendar_account: Current.user.writable_calendar_accounts,
          is_writable: true,
          syncing: true
        ).find_by(id: params[:calendar_id])

        unless calendar
          return render_api_error("calendar_not_writable",
                                  "The specified calendar is not writable or does not exist.",
                                  status: :forbidden)
        end

        event = calendar.calendar_events.new(event_params)
        event.assign_attributes(
          provider_event_id: "local-#{SecureRandom.uuid}",
          status: :confirmed,
          outbound_pending: true
        )
        event.save!
        Calendars::EventWriteJob.perform_later(event.id, "create")
        render_data(CalendarEventSerializer.new(event, detail: true).as_json, status: :created)
      end

      def update
        @event.update!(event_params.merge(outbound_pending: true))
        Calendars::EventWriteJob.perform_later(@event.id, "update", recurrence_scope)
        render_data(CalendarEventSerializer.new(@event, detail: true).as_json)
      end

      # Deletion is asynchronous: mark outbound_pending and enqueue the provider
      # delete. The local record is tombstoned (status: cancelled) by the job on
      # success. Returns 202 Accepted with the current event state.
      def destroy
        @event.update_columns(outbound_pending: true)
        Calendars::EventWriteJob.perform_later(@event.id, "delete", recurrence_scope)
        render_data(CalendarEventSerializer.new(@event, detail: true).as_json, status: :accepted)
      end

      def rsvp
        status = params[:rsvp_status]

        unless CalendarEvent.rsvp_statuses.key?(status)
          return render_api_error("invalid_rsvp_status",
                                  "rsvp_status must be one of: needs_action, accepted, declined, tentative.",
                                  status: :unprocessable_entity)
        end

        @event.update_columns(rsvp_status: CalendarEvent.rsvp_statuses[status], outbound_pending: true)
        Calendars::EventWriteJob.perform_later(@event.id, "rsvp")
        render_data(CalendarEventSerializer.new(@event, detail: true).as_json)
      end

      private

      def set_event
        @event = CalendarEvent.accessible_to(Current.user).find(params[:id])
      end

      # Mirrors the web require_writable_event gate. Renders 403 and halts the
      # callback chain when the event's calendar is not writable by the user.
      def require_writable_event!
        return if @event.calendar.is_writable && @event.calendar_account.writable_by?(Current.user)

        render_api_error("event_not_writable",
                         "You do not have write access to this calendar event.",
                         status: :forbidden)
      end

      def event_params
        params.permit(:title, :description, :location, :start_at, :end_at, :all_day, :color)
      end

      def recurrence_scope
        %w[this all].include?(params[:recurrence_scope]) ? params[:recurrence_scope] : "this"
      end

      def parse_time(value)
        Time.zone.parse(value) if value.present?
      rescue ArgumentError
        nil
      end
    end
  end
end
