# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Event CRUD, RSVP, and reschedule for the first-party calendar surface.
      # Mirrors the logic of the web CalendarEventsController and Api::V1::CalendarEventsController
      # without duplicating their mutation logic. Provider writes are async via Calendars::EventWriteJob.
      class CalendarEventsController < Api::App::BaseController
        before_action :set_event, only: %i[show update destroy rsvp reschedule]
        before_action :require_writable_event!, only: %i[update destroy rsvp]

        def show
          render_data(serialize_event(@event, detail: true))
        end

        def create
          calendar = writable_calendars.find_by(id: params[:calendar_id])
          unless calendar
            return render_error("calendar_not_writable",
                                "The specified calendar is not writable or does not exist.",
                                status: :forbidden)
          end

          event = calendar.calendar_events.new(event_params)
          event.assign_attributes(
            provider_event_id: "local-#{SecureRandom.uuid}",
            status: :confirmed,
            outbound_pending: true,
            is_organizer: true
          )
          apply_event_type(event)
          event.save!
          Calendars::EventWriteJob.perform_later(event.id, "create")
          render_data(serialize_event(event, detail: true), status: :created)
        end

        def update
          @event.assign_attributes(event_params.merge(outbound_pending: true))
          apply_event_type(@event)
          @event.save!
          Calendars::EventWriteJob.perform_later(@event.id, "update", recurrence_scope)
          render_data(serialize_event(@event, detail: true))
        end

        # Async delete: mark outbound_pending, enqueue the provider delete.
        # The local record is tombstoned (status: cancelled) by the job on success.
        def destroy
          @event.update_columns(outbound_pending: true)
          Calendars::EventWriteJob.perform_later(@event.id, "delete", recurrence_scope)
          render_data(serialize_event(@event, detail: true), status: :accepted)
        end

        def rsvp
          status = params[:rsvp_status]
          unless CalendarEvent.rsvp_statuses.key?(status)
            return render_error("invalid_rsvp_status",
                                "rsvp_status must be one of: needs_action, accepted, declined, tentative.",
                                status: :unprocessable_entity)
          end

          @event.update_columns(
            rsvp_status: CalendarEvent.rsvp_statuses[status],
            outbound_pending: true
          )
          Calendars::EventWriteJob.perform_later(@event.id, "rsvp")
          render_data(serialize_event(@event, detail: true))
        end

        # Drag-to-reschedule: move start/end (client preserves duration). Accepts
        # start_at / end_at as ISO8601 strings. Returns the updated event.
        def reschedule
          unless @event.calendar.is_writable && @event.calendar_account.writable_by?(current_user)
            return render_error("event_not_writable",
                                "You do not have write access to this calendar event.",
                                status: :forbidden)
          end

          @event.update!(
            start_at: params[:start_at],
            end_at: params[:end_at],
            outbound_pending: true
          )
          Calendars::EventWriteJob.perform_later(@event.id, "update", "this")
          render_data(serialize_event(@event, detail: true))
        end

        private

        def set_event
          @event = CalendarEvent.accessible_to(current_user).find(params[:id])
        end

        def require_writable_event!
          return if @event.calendar.is_writable && @event.calendar_account.writable_by?(current_user)

          render_error("event_not_writable",
                       "You do not have write access to this calendar event.",
                       status: :forbidden)
        end

        def writable_calendars
          ::Calendar.where(
            calendar_account: current_user.writable_calendar_accounts,
            is_writable: true,
            syncing: true
          )
        end

        def event_params
          params.permit(:title, :description, :location, :start_at, :end_at,
                        :all_day, :rrule, :attendee_emails)
        end

        # Translate an optional event_type_id param (nil, "none", or an id) into
        # the event's event_type + type_status, mirroring the web form's type_choice.
        def apply_event_type(event)
          return unless params.key?(:event_type_id)

          case params[:event_type_id].to_s
          when "", "none"
            event.event_type = nil
            event.type_status = :manual
          else
            type = current_workspace.event_types.find_by(id: params[:event_type_id])
            event.event_type = type
            event.type_status = type ? :manual : :pending
          end
        end

        def recurrence_scope
          %w[this all].include?(params[:recurrence_scope]) ? params[:recurrence_scope] : "this"
        end

        def serialize_event(event, detail: false)
          Api::App::Calendar::CalendarEventSerializer.new(event, detail: detail).as_json
        end
      end
    end
  end
end
