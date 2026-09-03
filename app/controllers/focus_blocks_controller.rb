# frozen_string_literal: true

# Keep / Move / dismiss a Scout focus block from its Time-agenda row. Keeping turns
# the block into a real calendar event (Time::FocusKeeper); moving shifts it to a
# chosen free slot; dismissing drops it. Each re-renders the agenda in place (so a
# kept block yields to its event and a moved one relocates) with a toast. Gated on
# Features.bold_layout?; personal records, so 404 (not 403) for another user's block.
class FocusBlocksController < ApplicationController
  include TimeAgendaLoading

  before_action :require_bold_layout_enabled
  before_action :set_focus_block

  def keep
    result = Time::FocusKeeper.call(@focus_block, user: current_user)
    if result.success?
      respond_with_agenda(result.calendar? ? t(".kept") : t(".kept_local"))
    else
      respond_with_error(result.error)
    end
  end

  def move
    slot = parse_slot(params[:start_at])
    return respond_with_error(t(".failed")) unless slot

    @focus_block.update!(start_at: slot, end_at: slot + @focus_block.duration_minutes.minutes, status: :moved)
    respond_with_agenda(t(".moved"))
  end

  def dismiss
    @focus_block.dismissed!
    respond_with_agenda(t(".dismissed"))
  end

  private

  # 404 (not 403) for a block that isn't the user's — matches the app convention.
  def set_focus_block
    @focus_block = FocusBlock.accessible_to(current_user).find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  def parse_slot(value)
    parsed = Time.zone.parse(value.to_s)
    parsed if parsed && parsed.future?
  rescue ArgumentError
    nil
  end

  # Re-render the whole agenda list (a focus action can move a row across days or
  # retire it), plus a toast. The agenda is always re-anchored on today — focus
  # blocks are near-term, so today's 30-day window always contains them.
  def respond_with_agenda(message)
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

  def agenda_list
    Campbooks::Time::AgendaList.new(
      items: @agenda, move_slots: @move_slots,
      snoozed_threads: @snoozed_threads, scheduled_emails: @scheduled_emails,
      zone: @zone, has_calendars: @has_calendars
    )
  end
end
