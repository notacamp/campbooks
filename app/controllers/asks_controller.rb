# frozen_string_literal: true

# The ask's three ways out, plus done/dismiss. An ask is a Task row with no page of
# its own: these actions are posted from a Now card, an email card's chip row, or a
# Time agenda row, and each re-renders the Time agenda in place (Turbo) or redirects
# to /time (HTML). The only decision an ask ever puts to the reader is *when* — Hold
# Scout's free slot, Set a date, or Not now — and all three land it on Time.
#
# Gated by Features.tasks? (readiness) and the :tasks entitlement (billing).
# Personal-to-the-workspace, so a miss 404s (never 403), matching the app convention.
class AsksController < ApplicationController
  include TimeAgendaLoading # load_time_agenda / agenda_list / agenda_move_slots

  before_action :require_authentication
  before_action :require_tasks_enabled
  before_action :set_ask
  before_action :require_tasks_entitlement

  # Hold Scout's earliest free slot for the ask (a kept focus block → a real
  # calendar event when a writable calendar exists, local otherwise).
  def hold
    result = Time::FocusHolder.call(@ask, user: current_user)
    if result.success?
      when_label = format_slot(result.slot)
      respond_with_agenda(result.calendar? ? t(".held", when: when_label) : t(".held_local", when: when_label))
    else
      respond_with_error(result.error)
    end
  end

  # Give the ask a due date — a preset (today / tomorrow / friday / next_week) or an
  # ISO date — resolved in the user's zone, at 09:30 local.
  def schedule
    date = resolve_date(params[:on])
    return respond_with_error(t(".invalid_date")) unless date

    @ask.schedule!(date, zone: current_user.effective_time_zone, by: current_user)
    respond_with_agenda(t(".scheduled", date: I18n.l(date, format: :long)))
  end

  # "Not now" — a week's snooze; the ask drops off every surface until it lapses.
  def snooze
    @ask.snooze!(by: current_user)
    respond_with_agenda(t(".snoozed"))
  end

  def done
    @ask.move_to_status!(:done, by: current_user)
    respond_with_agenda(t(".done"))
  end

  # Dismiss a suggested ask (→ cancelled). Allowed for any live ask; only offered in
  # the UI for still-suggested ones.
  def dismiss
    @ask.move_to_status!(:cancelled, by: current_user)
    respond_with_agenda(t(".dismissed"))
  end

  private

  # 404 (not 403) for an ask the user can't access — matches the app convention.
  def set_ask
    @ask = Task.accessible_to(current_user).find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # Safety net for a direct POST when the plan doesn't allow tasks (the UI hides the
  # controls). Renders the upgrade response and halts the action.
  def require_tasks_entitlement
    require_entitlement!(:tasks)
  end

  def resolve_date(param)
    Asks::PresetDate.resolve(param, current_user.effective_time_zone)
  end

  def format_slot(time)
    I18n.l(time.in_time_zone(current_user.effective_time_zone), format: "%A %H:%M")
  end

  # Reload the whole agenda (an ask action can move a row across days, retire it, or
  # spawn a focus row) and replace it in place, with a toast. HTML falls back to a
  # redirect. The Task callbacks already refresh the feed; enqueue once more,
  # best-effort, so the Now cards reconcile promptly.
  def respond_with_agenda(message)
    refresh_feed
    load_time_agenda
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: [
          turbo_stream.replace("time_agenda", render_to_string(agenda_list, layout: false)),
          notify_stream(message)
        ]
      end
      format.html { redirect_to time_path, success: message }
    end
  end

  def respond_with_error(message)
    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :error), status: :unprocessable_entity }
      format.html { redirect_to time_path, alert: message }
    end
  end

  def refresh_feed
    Feed::RefreshJob.enqueue_for_workspace(Current.workspace)
  rescue StandardError => e
    Rails.logger.warn("[AsksController] feed refresh failed: #{e.class}: #{e.message}")
  end
end
