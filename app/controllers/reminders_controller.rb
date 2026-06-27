# The dedicated reminders surface: every AI-extracted reminder grouped by state,
# with confirm / snooze / dismiss. Confirming creates the calendar event (and
# optionally adjusts the date first — LLM dates are sometimes wrong).
class RemindersController < ApplicationController
  include ActionView::RecordIdentifier # dom_id(reminder) ⇒ "reminder_<id>"

  before_action :require_authentication
  before_action :set_reminder, only: %i[confirm dismiss snooze]
  # Reminders live inside the Calendar nav item, so visiting them clears the
  # shared :calendar dot (which also covers new pending reminders).

  def index
    scope = Reminder.accessible_to(current_user)
    # Past-due reminders aren't actionable, so the page only offers today-forward
    # ones (all-day reminders due today included via the start-of-day floor).
    @upcoming  = scope.pending.where(due_at: Time.current.beginning_of_day..).order(:due_at)
    @snoozed   = scope.snoozed.order(:snoozed_until)
    @confirmed = scope.confirmed.order(due_at: :desc).limit(20)
  end

  def confirm
    apply_date_edit
    result = Reminders::Confirm.call(@reminder, user: current_user)
    if result.success?
      respond_with_change(result.calendar? ? t(".confirmed") : t(".confirmed_no_calendar"))
    else
      respond_with_error(result.error)
    end
  end

  def dismiss
    @reminder.dismissed!
    Events.publish("reminder.dismissed", subject: @reminder, payload: { "title" => @reminder.title, "due_at" => @reminder.due_at&.iso8601 })
    respond_with_change(t(".dismissed"))
  end

  def snooze
    @reminder.update!(status: :snoozed, snoozed_until: snooze_until)
    respond_with_change(t(".snoozed"))
  end

  private

  # 404 (not 403) for reminders the user can't access — matches the app convention.
  def set_reminder
    @reminder = Reminder.accessible_to(current_user).find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # Optional inline correction of the date/time before confirming.
  def apply_date_edit
    return if params[:due_at].blank?
    parsed = Time.zone.parse(params[:due_at].to_s)
    @reminder.update!(due_at: parsed) if parsed
  rescue ArgumentError
    nil
  end

  def snooze_until
    return 1.week.from_now if params[:until].blank?
    Time.zone.parse(params[:until].to_s) || 1.week.from_now
  rescue ArgumentError
    1.week.from_now
  end

  def respond_with_change(message)
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [ turbo_stream.remove(dom_id(@reminder)), notify_stream(message) ]
      end
      format.html { redirect_to reminders_path, success: message }
    end
  end

  def respond_with_error(message)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :error), status: :unprocessable_entity }
      format.html { redirect_to reminders_path, error: message }
    end
  end
end
