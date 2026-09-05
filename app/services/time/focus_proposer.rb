# frozen_string_literal: true

# Proposes focus blocks for the commitments Scout should hold work time for: the
# deadlines it found in mail (pending reminders) AND the accepted, dated asks. For
# each — a deadline reminder of a type worth holding time for (deadline /
# payment_due / renewal / other, never an appointment you merely attend), or an
# accepted ask with a due date — due in the next 7 days and without a block yet, it
# finds the earliest free 45-minute slot between tomorrow 09:00 and the deadline
# minus two hours (Time::SlotFinder over Time::BusyIntervals) and stakes a
# `proposed` FocusBlock there.
#
# Idempotent: one block per reminder ever (the unique index) and one per ask ever
# (an exists? guard — asks have no unique index), so re-running — as TimeController
# does on each visit, debounced — never duplicates, and a dismissed block is never
# re-proposed. Suggested (untriaged) asks are never auto-proposed.
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
    eligible_tasks.each { |task| propose_for_task(task) } if Features.tasks?
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

  # Accepted, dated asks due within the lookahead that don't already hold a block —
  # one block per ask ever (a dismissed block is not re-proposed), like reminders.
  # Suggested (untriaged) asks are never auto-proposed: holding time for an ask
  # Scout only guessed at would clutter the calendar. `.live.where(status:
  # ACTIVE_STATUSES)` = accepted, awake, not archived.
  def eligible_tasks
    horizon = (@today + LOOKAHEAD).end_of_day
    ::Task.accessible_to(@user).live.where(status: ::Task::ACTIVE_STATUSES).dated
          .where(due_at: ::Time.current..horizon)
          .where.not(id: FocusBlock.where.not(task_id: nil).select(:task_id))
          .order(:due_at)
  end

  def propose_for(reminder)
    propose(subject: :reminder, record: reminder, deadline: reminder.due_at,
            reason: "deadline_due_#{reminder.due_at.to_date.iso8601}")
  rescue ActiveRecord::RecordNotUnique
    # A concurrent proposer already placed this reminder's block — fine, skip.
    nil
  end

  # Tasks have no unique index on focus_blocks.task_id (an ask may be held again
  # after a dismissed block), so guard the race with an exists? check rather than
  # the RecordNotUnique rescue reminders rely on.
  def propose_for_task(task)
    return if FocusBlock.for_task(task).exists?

    propose(subject: :task, record: task, deadline: task.due_at,
            reason: "ask_due_#{task.due_at.to_date.iso8601}")
  end

  def propose(subject:, record:, deadline:, reason:)
    from = tomorrow_nine
    to = deadline - DEADLINE_BUFFER
    return if from >= to

    slot = Time::SlotFinder.find(
      from: from, to: to, duration_minutes: DURATION_MINUTES,
      busy: Time::BusyIntervals.for(@user, from, to), zone: @zone
    )
    return unless slot

    @created << FocusBlock.create!(
      workspace: @user.workspace, user: @user, subject => record,
      title: I18n.t("time.focus.title", subject: record.title),
      start_at: slot, end_at: slot + DURATION_MINUTES.minutes,
      status: :proposed, reason: reason
    )
  end

  def tomorrow_nine
    date = @today + 1
    @zone.local(date.year, date.month, date.day, Time::SlotFinder::WORK_START_HOUR)
  end
end
