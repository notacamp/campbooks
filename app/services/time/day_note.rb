# frozen_string_literal: true

# Scout's day note at the top of the bold Time agenda — pure, no LLM. It reads
# today's rows and reports only what they prove (docs/messaging.md: honest, never
# invents): how many meetings and deadlines today, the first deadline, the next
# focus block Scout is holding, and — only when the Money PR's Money::Ledger is
# present — the most overdue obligation. The component turns these facts into
# sentence(s) via i18n; here there is no copy, just the counts and titles.
class Time::DayNote
  Result = Data.define(:date, :meetings_count, :deadlines_count, :first_deadline_title, :focus, :late_obligation, :undated_count) do
    def any?
      meetings_count.positive? || deadlines_count.positive? || !focus.nil? ||
        !late_obligation.nil? || undated_count.positive?
    end
  end

  Focus = Data.define(:title, :subject, :at, :duration_minutes)
  LateObligation = Data.define(:name, :days_overdue)

  def self.for(user, date: nil)
    new(user, date).call
  end

  def initialize(user, date)
    @user = user
    @zone = user.effective_time_zone
    @today = date || ::Time.current.in_time_zone(@zone).to_date
  end

  def call
    Result.new(
      date: @today,
      meetings_count: meetings_count,
      deadlines_count: today_deadlines.size,
      first_deadline_title: today_deadlines.first&.title,
      focus: next_focus,
      late_obligation: late_obligation,
      undated_count: undated_count
    )
  end

  private

  # Live undated asks Scout isn't already holding time for — the "N asks have no
  # date yet" tail of the note. Matches Time::Agenda#undated's set.
  def undated_count
    return 0 unless Features.tasks?

    Task.accessible_to(@user).live.undated
        .where.not(id: FocusBlock.held.where.not(task_id: nil).select(:task_id))
        .count
  end

  def day_range
    from = @zone.local(@today.year, @today.month, @today.day).beginning_of_day
    from..from.end_of_day
  end

  # Timed events today (all-day items aren't "meetings"), on syncing calendars the
  # user hasn't hidden — the same set the agenda draws.
  def meetings_count
    base = CalendarEvent.accessible_to(@user).visible.where(all_day: false)
                        .where(calendars: { syncing: true })
    hidden = Array(@user.hidden_calendar_ids)
    base = base.where.not(calendar_id: hidden) if hidden.any?
    base.where(start_at: day_range).count
  end

  def today_deadlines
    @today_deadlines ||= Reminder.accessible_to(@user).pending.where(calendar_event_id: nil)
                                 .where(due_at: day_range).order(:due_at).to_a
  end

  # The soonest focus block Scout is holding from now forward (proposed / moved /
  # kept), for "I held 45 minutes tomorrow at 10 for the Q3 deck comments".
  def next_focus
    block = FocusBlock.accessible_to(@user).held.where(start_at: ::Time.current..).order(:start_at).first
    return nil unless block

    Focus.new(
      title: block.title,
      subject: block.subject_title,
      at: block.start_at,
      duration_minutes: block.duration_minutes
    )
  end

  # The most overdue money obligation — only when the Money PR is present. On this
  # branch Money::Ledger doesn't exist, so this is always nil (the note omits the
  # sentence), per the spec's defined?(Money::Ledger) guard.
  def late_obligation
    return nil unless defined?(Money::Ledger)

    entry = safe_most_overdue
    return nil unless entry

    LateObligation.new(name: entry[:name], days_overdue: entry[:days_overdue])
  end

  def safe_most_overdue
    return nil unless Money::Ledger.respond_to?(:most_overdue_for)

    Money::Ledger.most_overdue_for(@user)
  rescue StandardError
    nil
  end
end
