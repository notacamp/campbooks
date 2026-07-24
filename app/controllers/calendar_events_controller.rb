class CalendarEventsController < ApplicationController
  include ActionView::RecordIdentifier

  before_action :require_authentication
  before_action :set_event, only: [ :show, :edit, :update, :destroy, :rsvp, :reschedule ]
  before_action :require_writable_event, only: [ :update, :destroy, :rsvp ]

  def show
    redirect_to edit_calendar_event_path(@event)
  end

  def new
    @event = CalendarEvent.new(prefilled_event_attrs)
    @calendars = writable_calendars
  end

  def create
    calendar = writable_calendars.find_by(id: params.dig(:calendar_event, :calendar_id))
    return redirect_to(calendar_path, error: t(".no_calendar")) unless calendar

    @event = calendar.calendar_events.new(event_params.except(:calendar_id))
    # A temp id satisfies the (calendar_id, provider_event_id) unique index until
    # the provider assigns the real one (EventWriter swaps it in on create).
    # is_organizer is asserted up front (the provider would confirm it on the
    # next pull anyway) so the guests editor is available immediately.
    @event.assign_attributes(provider_event_id: "local-#{SecureRandom.uuid}", status: :confirmed, outbound_pending: true, is_organizer: true)
    apply_type_choice(@event)

    if @event.save(context: :form)
      Calendars::EventWriteJob.perform_later(@event.id, "create")
      enqueue_classification(@event)
      Events.publish("calendar_event.created", subject: @event, workspace: @event.calendar.workspace, payload: { "title" => @event.title, "starts_at" => @event.start_at&.iso8601 })
      respond_to do |format|
        format.turbo_stream { flash[:success] = t(".created"); render_event_saved }
        format.html { redirect_to calendar_path(view: params[:view], date: @event.start_at.to_date.iso8601), success: t(".created") }
      end
    else
      @calendars = writable_calendars
      render :new, status: :unprocessable_entity
    end
  end

  def edit
    @calendars = writable_calendars
  end

  def update
    @event.assign_attributes(event_params.except(:calendar_id).merge(outbound_pending: true))
    apply_type_choice(@event)
    if @event.save(context: :form)
      Calendars::EventWriteJob.perform_later(@event.id, "update", recurrence_scope)
      enqueue_classification(@event)
      Events.publish("calendar_event.updated", subject: @event, workspace: @event.calendar.workspace, payload: { "title" => @event.title, "starts_at" => @event.start_at&.iso8601 })
      respond_to do |format|
        format.turbo_stream { flash[:success] = t(".updated"); render_event_saved }
        format.html { redirect_to calendar_path(view: params[:view], date: @event.start_at.to_date.iso8601), success: t(".updated") }
      end
    else
      @calendars = writable_calendars
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @event.update_columns(outbound_pending: true)
    Calendars::EventWriteJob.perform_later(@event.id, "delete", recurrence_scope)
    Events.publish("calendar_event.deleted", subject: @event, workspace: @event.calendar.workspace, payload: { "title" => @event.title })
    respond_to do |format|
      format.turbo_stream do
        if params[:swipe].present?
          render turbo_stream: [ turbo_stream.remove(dom_id(@event, :agenda_item)), notify_stream(t(".deleted")) ]
        else
          flash[:success] = t(".deleted")
          render_event_saved
        end
      end
      format.html { redirect_to calendar_path(view: params[:view]), success: t(".deleted") }
    end
  end

  def rsvp
    unless CalendarEvent.rsvp_statuses.key?(params[:rsvp_status])
      return redirect_to(calendar_path, error: t(".invalid_rsvp"))
    end
    @event.update_columns(rsvp_status: CalendarEvent.rsvp_statuses[params[:rsvp_status]], outbound_pending: true)
    Calendars::EventWriteJob.perform_later(@event.id, "rsvp")
    respond_to do |format|
      format.turbo_stream { flash[:success] = t(".rsvp_saved"); render_event_saved }
      format.html { redirect_to calendar_path(view: params[:view], date: @event.start_at.to_date.iso8601), success: t(".rsvp_saved") }
    end
  end

  # Drag-to-reschedule from the time grids: move an event's start/end (duration
  # preserved by the client). JSON in, head out, so the Stimulus controller can
  # tell success (200) from a permission denial (403) and revert on failure.
  def reschedule
    unless @event.calendar.is_writable && @event.calendar_account.writable_by?(Current.user)
      return head :forbidden
    end

    if @event.update(start_at: params[:start_at], end_at: params[:end_at], outbound_pending: true)
      Calendars::EventWriteJob.perform_later(@event.id, "update", "this")
      head :ok
    else
      head :unprocessable_entity
    end
  end

  private

  def set_event
    # 404 (not 403) for events the user can't see — don't leak existence.
    @event = CalendarEvent.accessible_to(Current.user).find(params[:id])
  end

  def require_writable_event
    return if @event.calendar.is_writable && @event.calendar_account.writable_by?(Current.user)
    redirect_to calendar_path, error: t("calendar_events.errors.not_writable")
  end

  # Turbo Stream for a successful modal save/delete: navigate the whole page to the
  # calendar at the event's date (breaking out of the modal frame) so the change is
  # visible. The flash the caller set shows on that next page load.
  def render_event_saved
    render turbo_stream: turbo_stream.append(
      "calendar_event_modal",
      partial: "calendar_events/navigate",
      locals: { url: calendar_path(view: params[:view].presence, date: @event.start_at.to_date.iso8601) }
    )
  end

  # Calendars the user may create/edit events on: write-shared accounts, and the
  # calendar itself flagged writable by the provider and turned on for sync.
  def writable_calendars
    Calendar.where(calendar_account: Current.user.writable_calendar_accounts, is_writable: true, syncing: true)
            .includes(:calendar_account).order(is_primary: :desc, name: :asc)
  end

  def event_params
    permitted = params.require(:calendar_event)
                      .permit(:title, :description, :location, :start_at, :end_at, :all_day, :calendar_id, :rrule,
                              :attendee_emails, :start_date, :start_time, :end_date, :end_time)
    # Only the organizer may change the guest list (matches what the form
    # offers; enforced here so a crafted request can't either).
    permitted.delete(:attendee_emails) unless guests_editable?
    compose_times(permitted)
  end

  # The form submits date and time as separate chip inputs; compose them into
  # start_at/end_at here (no-JS safe). Whole start_at/end_at params still work
  # unchanged (reschedule, API-ish callers, older tests). All-day events store
  # end_at as an EXCLUSIVE midnight — the Google/ICS convention the inbound
  # sync already uses — so "16–17 Jul" submits as end_at 18 Jul 00:00.
  def compose_times(permitted)
    start_date = permitted.delete(:start_date)
    start_time = permitted.delete(:start_time)
    end_date   = permitted.delete(:end_date)
    end_time   = permitted.delete(:end_time)
    return permitted if start_date.blank?

    if ActiveModel::Type::Boolean.new.cast(permitted[:all_day])
      permitted[:start_at] = parse_param_date(start_date)
      last_day = parse_param_date(end_date) || permitted[:start_at]
      permitted[:end_at] = last_day && (last_day + 1.day)
    else
      permitted[:start_at] = parse_param_time("#{start_date} #{start_time}")
      permitted[:end_at]   = parse_param_time("#{end_date.presence || start_date} #{end_time}")
    end
    permitted
  end

  # Guests are editable on a brand-new event or one the user organizes. An
  # invite they merely received shows the read-only guest list instead.
  def guests_editable?
    @event.nil? || @event.new_record? || @event.is_organizer?
  end
  helper_method :guests_editable?

  # Translate the form's single "Type" selector into event_type + type_status:
  #   "auto"/blank → pending (AI classifies + colors it in the background)
  #   "none"       → manual, untyped (keeps the calendar color)
  #   "<id>"       → manual, that workspace type (scoped, so no cross-workspace ids)
  def apply_type_choice(event)
    case params[:type_choice].to_s
    when "", "auto"
      event.event_type = nil
      event.type_status = :pending
    when "none"
      event.event_type = nil
      event.type_status = :manual
    else
      type = Current.workspace.event_types.find_by(id: params[:type_choice])
      event.event_type = type
      event.type_status = type ? :manual : :pending
    end
  end

  def enqueue_classification(event)
    EventClassificationJob.set(wait: 10.seconds).perform_later(event.id) if event.type_status_pending?
  end

  def recurrence_scope
    params[:recurrence_scope].in?(%w[this all]) ? params[:recurrence_scope] : "this"
  end

  def default_start
    (Time.current + 1.hour).change(min: 0)
  end

  # Honor ?start/?end/?date/?all_day from click- and drag-to-create on the grids,
  # falling back to the next round hour.
  def prefilled_event_attrs
    start = parse_param_time(params[:start]) || parse_param_date(params[:date])&.change(hour: 9) || default_start
    all_day = params[:all_day] == "true"
    finish = parse_param_time(params[:end]) || (all_day ? start.end_of_day : start + 1.hour)
    { start_at: start, end_at: finish, all_day: all_day, calendar: writable_calendars.first }
  end

  def parse_param_time(value)
    Time.zone.parse(value) if value.present?
  rescue ArgumentError
    nil
  end

  def parse_param_date(value)
    Date.parse(value).in_time_zone if value.present?
  rescue ArgumentError
    nil
  end
end
