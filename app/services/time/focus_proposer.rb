# frozen_string_literal: true

# Proposes focus blocks for the deadlines Scout found in mail. For each pending
# deadline reminder — of a type worth holding work time for (deadline / payment_due
# / renewal / other, never an appointment you merely attend) — due in the next 7
# days and without a block yet, it finds the earliest free 45-minute slot between
# tomorrow 09:00 and the deadline minus two hours (Time::SlotFinder, avoiding busy
# events and other blocks) and stakes a `proposed` FocusBlock there.
#
# Idempotent: one block per reminder ever (the unique index), so re-running — as
# TimeController does on each visit, debounced — never duplicates, and a dismissed
# block is never re-proposed.
class Time::FocusProposer
  DURATION_MINUTES = 45
  LOOKAHEAD = 7.days
  DEADLINE_BUFFER = 2.hours
  ELIGIBLE_TYPES = %w[deadline payment_due renewal other].freeze

  def self.for(user, today: nil)
    new(user, today:).call
  end

  def initialize(user, today: nil)
    @user = user
    @zone = user.effective_time_zone
    @today = today || ::Time.current.in_time_zone(@zone).to_date
    @created = []
  end

  def call
    eligible_deadlines.each { |reminder| propose_for(reminder) }
    @created
  end

  private

  def eligible_deadlines
    horizon = (@today + LOOKAHEAD).end_of_day
    Reminder.accessible_to(@user).pending
            .where(calendar_event_id: nil)
            .where(reminder_type: ELIGIBLE_TYPES)
            .where(due_at: ::Time.current..horizon)
            .where.not(id: FocusBlock.where.not(reminder_id: nil).select(:reminder_id))
            .order(:due_at)
  end

  def propose_for(reminder)
    from = tomorrow_nine
    to = reminder.due_at - DEADLINE_BUFFER
    return if from >= to

    slot = Time::SlotFinder.find(
      from: from, to: to, duration_minutes: DURATION_MINUTES,
      busy: busy_intervals(from, to), zone: @zone
    )
    return unless slot

    @created << FocusBlock.create!(
      workspace: @user.workspace, user: @user, reminder: reminder,
      title: I18n.t("time.focus.title", subject: reminder.title),
      start_at: slot, end_at: slot + DURATION_MINUTES.minutes,
      status: :proposed, reason: "deadline_due_#{reminder.due_at.to_date.iso8601}"
    )
  rescue ActiveRecord::RecordNotUnique
    # A concurrent proposer already placed this reminder's block — fine, skip.
    nil
  end

  def tomorrow_nine
    date = @today + 1
    @zone.local(date.year, date.month, date.day, Time::SlotFinder::WORK_START_HOUR)
  end

  # Timed, non-cancelled events in the window plus other held focus blocks — the
  # things a new block must not overlap.
  def busy_intervals(from, to)
    events = CalendarEvent.accessible_to(@user).visible.where(all_day: false)
                          .in_range(from, to)
                          .pluck(:start_at, :end_at)
                          .map { |s, e| [ s, e || (s + 30.minutes) ] }
    blocks = FocusBlock.accessible_to(@user).held.where(start_at: from..to).pluck(:start_at, :end_at)
    events + blocks
  end
end
