# frozen_string_literal: true

# On-demand "Hold time" for an ask: the same focus-block machinery Time::FocusProposer
# stakes for a deadline, but triggered by the user picking "Hold" on a Now card or a
# Time row rather than by the background sweep. Finds the earliest free working-hours
# slot (Time::SlotFinder over Time::BusyIntervals), stakes a `proposed` FocusBlock on
# the ask, Keeps it immediately (Time::FocusKeeper → a real calendar event when a
# writable calendar exists, local otherwise), and accepts the ask.
#
# `#preview` does the slot search only (no writes) so cards/rows can label the button
# "Hold Thu 10:00" — it shares Time::BusyIntervals' per-request cache, so a deck of
# cards costs one calendar query.
#
# Idempotent: an ask that already holds a block returns that block untouched, so a
# double-tap never stakes two.
class Time::FocusHolder
  DURATION_MINUTES = Time::FocusProposer::DURATION_MINUTES

  Result = Data.define(:success, :focus_block, :slot, :calendar_event, :error) do
    def success? = success
    def calendar? = !calendar_event.nil?
  end

  # The slot "Hold" would take, without writing anything (nil when none is free).
  def self.preview(task, user:)
    new(task, user).preview
  end

  def self.call(task, user:)
    new(task, user).call
  end

  def initialize(task, user)
    @task = task
    @user = user
    @zone = user.effective_time_zone
  end

  def preview
    slot
  end

  def call
    existing = @task.held_block
    if existing
      return Result.new(success: true, focus_block: existing, slot: existing.start_at,
                        calendar_event: existing.calendar_event, error: nil)
    end

    chosen = slot
    return failure(I18n.t("asks.hold.no_slot")) unless chosen

    block = FocusBlock.create!(
      workspace: @user.workspace, user: @user, task: @task,
      title: I18n.t("time.focus.title", subject: @task.title),
      start_at: chosen, end_at: chosen + DURATION_MINUTES.minutes,
      status: :proposed, reason: "ask_held"
    )
    kept = Time::FocusKeeper.call(block, user: @user)
    @task.accept!(by: @user)
    Result.new(success: true, focus_block: block, slot: chosen,
               calendar_event: kept.calendar_event, error: nil)
  rescue ActiveRecord::RecordInvalid => e
    failure(e.message)
  end

  private

  HORIZON = 14.days

  # The earliest free 45-minute slot from an hour out to the ask's due date (minus
  # the deadline buffer), capped at a fortnight out. SlotFinder clamps to working
  # hours and prefers 10:00. Memoized so #preview and #call agree.
  #
  # The search start is rounded up to the quarter hour and the busy set is always
  # read for the full fortnight window, so every ask previewed in one request
  # shares ONE Time::BusyIntervals cache entry (a superset of busy intervals is
  # harmless: SlotFinder only checks overlaps).
  def slot
    return @slot if defined?(@slot)

    from = quarter_hour_after(::Time.current + 1.hour)
    horizon = from + HORIZON
    to = @task.due_at ? [ @task.due_at - Time::FocusProposer::DEADLINE_BUFFER, horizon ].min : horizon
    @slot =
      if from >= to
        nil
      else
        Time::SlotFinder.find(
          from: from, to: to, duration_minutes: DURATION_MINUTES,
          busy: Time::BusyIntervals.for(@user, from, horizon), zone: @zone
        )
      end
  end

  def quarter_hour_after(time)
    step = 15.minutes.to_i
    ::Time.at((time.to_i / step.to_f).ceil * step).in_time_zone(@zone)
  end

  def failure(error)
    Result.new(success: false, focus_block: nil, slot: nil, calendar_event: nil, error: error)
  end
end
