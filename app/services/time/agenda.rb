# frozen_string_literal: true

# Merges everything with a time into one chronological agenda for the bold Time
# surface: calendar events (recurring series expanded into occurrences), deadlines
# Scout found in mail (pending Reminders with no calendar event yet), due-dated
# active Tasks, and held FocusBlocks. Returns a flat, display-sorted list of
# Time::AgendaItem — the Rethink's core claim that a meeting, a deadline and a
# reminder are the same thing (a commitment with a time) made literal.
#
# Sorted: days ascending; within a day all-day items first, then by instant.
# Overdue deadlines are PINNED into today (never silently dropped). Bucketing and
# "today" are computed in the user's effective time zone.
class Time::Agenda
  include Rails.application.routes.url_helpers

  # How far back an unactioned overdue deadline can be and still be pinned into
  # today — keeps ancient pending reminders from crowding the list.
  OVERDUE_LOOKBACK = 45.days

  def self.for(user, from:, to:)
    new(user, from:, to:).items
  end

  def initialize(user, from:, to:)
    @user = user
    @from = from
    @to = to
    @zone = user.effective_time_zone
    @today = ::Time.current.in_time_zone(@zone).to_date
  end

  def items
    (event_items + deadline_items + task_items + focus_items).sort_by(&:sort_key)
  end

  private

  # ── Events ────────────────────────────────────────────────────────────────
  def event_items
    concrete = base_events.concrete.where(start_at: @from..@to).order(:start_at)
    expanded = Calendars::OccurrenceExpander.new(
      concrete: concrete, masters: base_events.series_masters, from: @from, to: @to
    ).events
    expanded.map { |event| event_item(event) }
  end

  def base_events
    @base_events ||= begin
      base = CalendarEvent.accessible_to(@user).visible
                          .where(calendars: { syncing: true })
                          .includes(:event_type, calendar: :calendar_account)
      hidden = Array(@user.hidden_calendar_ids)
      hidden.any? ? base.where.not(calendar_id: hidden) : base
    end
  end

  def event_item(event)
    label, path = source_for(email: event.source_email_message) ||
                  [ provider_label(event), nil ]
    Time::AgendaItem.new(
      kind: :event, at: event.start_at, day: day_of(event.start_at),
      all_day: event.all_day, overdue: false,
      duration_minutes: duration_minutes(event.start_at, event.end_at, event.all_day),
      title: event.title.presence || I18n.t("time.agenda.untitled"),
      source_label: label, source_path: path,
      color: event.display_color, record: event,
      actions: event_actions(event, path)
    )
  end

  def event_actions(event, source_path)
    return [ :meet ] if event.join_url.present?
    return [ :open_thread ] if source_path

    []
  end

  # ── Deadlines (pending reminders with no calendar event) ────────────────────
  def deadline_items
    forward = reminders_scope.where(due_at: @from..@to)
    overdue = overdue_reminders
    forward.map { |r| deadline_item(r, overdue: false) } +
      overdue.map { |r| deadline_item(r, overdue: true) }
  end

  def reminders_scope
    Reminder.accessible_to(@user).pending.where(calendar_event_id: nil).order(:due_at)
  end

  # Past-due deadlines to pin into today — only when the window actually opens on
  # the present (looking at today, not a future page), and not older than the
  # lookback so the list stays honest and bounded.
  def overdue_reminders
    return Reminder.none unless @from <= ::Time.current

    reminders_scope.where(due_at: (@from - OVERDUE_LOOKBACK)...@from)
  end

  def deadline_item(reminder, overdue:)
    label, path = source_for(email: reminder.source_email) ||
                  source_for(document: reminder_document(reminder)) ||
                  [ nil, nil ]
    actions = []
    actions << :open_thread if reminder.source_email
    actions << :open_document if path && reminder.source_email.nil?
    actions << :add_to_calendar
    Time::AgendaItem.new(
      kind: :deadline, at: reminder.due_at, day: overdue ? @today : day_of(reminder.due_at),
      all_day: reminder.all_day?, overdue: overdue, duration_minutes: 0,
      title: reminder.title, source_label: label, source_path: path,
      color: nil, record: reminder, actions: actions
    )
  end

  def reminder_document(reminder)
    reminder.source if reminder.source_type == "Document"
  end

  # ── Tasks (active, due-dated; suggested excluded by the active scope) ────────
  def task_items
    return [] unless Features.tasks?

    tasks = Task.accessible_to(@user).active.where.not(due_at: nil).where(due_at: @from..@to)
    tasks.map { |task| task_item(task) }
  end

  def task_item(task)
    label, path = source_for(email: task.source_email) ||
                  [ I18n.t(task.ai_suggested? ? "time.agenda.source.scout_suggested" : "time.agenda.source.task"), nil ]
    Time::AgendaItem.new(
      kind: :task, at: task.due_at, day: day_of(task.due_at),
      all_day: task.all_day, overdue: false, duration_minutes: 0,
      title: task.title, source_label: label, source_path: path,
      color: nil, record: task, actions: [ :done ]
    )
  end

  # ── Focus blocks (held: proposed / moved / kept-but-local) ──────────────────
  def focus_items
    FocusBlock.accessible_to(@user).renderable.where(start_at: @from..@to).map { |fb| focus_item(fb) }
  end

  def focus_item(block)
    Time::AgendaItem.new(
      kind: :focus, at: block.start_at, day: day_of(block.start_at),
      all_day: false, overdue: false, duration_minutes: block.duration_minutes,
      title: block.title,
      source_label: I18n.t("time.agenda.source.scout_suggested"), source_path: nil,
      color: nil, record: block, actions: [ :move, :keep, :dismiss_focus ]
    )
  end

  # ── Shared helpers ──────────────────────────────────────────────────────────
  def day_of(time)
    time.in_time_zone(@zone).to_date
  end

  def duration_minutes(start_at, end_at, all_day)
    return 0 if all_day || end_at.blank? || start_at.blank?

    ((end_at - start_at) / 60).round
  end

  # The unified "from <sender>'s email" / "from <document>" back-link, or nil when
  # the record has no such source. Returns [label, path] so a nil short-circuits
  # the `|| fallback` in each builder.
  def source_for(email: nil, document: nil)
    if email
      [ I18n.t("time.agenda.source.from_email", name: sender_first_name(email)), email_message_path(email) ]
    elsif document
      [ I18n.t("time.agenda.source.from_document", title: document.display_title.presence || I18n.t("time.agenda.untitled")),
        document_path(document) ]
    end
  end

  def sender_first_name(email)
    Emails::SenderName.first_name(email.from_address).presence || I18n.t("time.agenda.source.someone")
  end

  def provider_label(event)
    provider = event.calendar&.calendar_account&.provider
    key = %w[google zoho].include?(provider) ? "time.agenda.source.#{provider}_calendar" : "time.agenda.source.calendar"
    I18n.t(key)
  end
end
