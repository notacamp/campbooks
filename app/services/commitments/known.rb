module Commitments
  # Assembles a list of one-line summaries of commitments the workspace already
  # tracks, for injection into extraction prompts. The model uses this as an
  # exclusion list to avoid re-extracting what is already on the reader's calendar,
  # task list, or reminder list.
  #
  # Best-effort: the entire body is wrapped so any error logs a warning and returns
  # [] — a failure here must never block staging or crash an extraction job.
  module Known
    module_function

    # Returns an Array<String> of deduplicated one-line summaries, capped at `limit`
    # total. Thread-mates come FIRST (they're the strongest dedup signal — the same
    # conversation restating the same commitment), then workspace-wide window items
    # in date order; the cap trims the window items, never the thread-mates.
    def for(workspace:, source:, horizon: 90.days, limit: 30)
      (thread_lines(source, limit) + window_lines(workspace, horizon, limit))
        .uniq.first(limit)
    rescue => e
      Rails.logger.warn("[Commitments::Known] failed for #{source.class}##{source.try(:id)}: #{e.message}")
      []
    end

    def thread_lines(source, limit)
      # Thread-mates: tasks and reminders already staged from any other message in the
      # same email conversation. All statuses included — a dismissed reminder should
      # still suppress a duplicate from a follow-up. Only for EmailMessage sources with
      # a thread.
      return [] unless source.is_a?(EmailMessage) && source.email_thread_id.present?

      thread_msg_ids = EmailMessage.where(email_thread_id: source.email_thread_id).select(:id)

      task_lines = Task.where(source_type: "EmailMessage", source_id: thread_msg_ids)
                       .order(created_at: :desc).limit(limit)
                       .map { |t| task_line(t) }

      reminder_lines = Reminder.where(source_type: "EmailMessage", source_id: thread_msg_ids)
                               .order(created_at: :desc).limit(limit)
                               .map { |r| reminder_line(r) }

      task_lines + reminder_lines
    end

    def window_lines(workspace, horizon, limit)
      # Workspace-wide window: pending/active commitments from today through the
      # horizon, each kind date-ordered and capped so a busy calendar can't crowd
      # out nearer, more collision-prone items.
      window = Date.current.beginning_of_day..(Date.current + horizon).end_of_day

      reminder_lines = Reminder
        .where(workspace: workspace, status: %i[pending confirmed snoozed])
        .where(due_at: window)
        .order(:due_at).limit(limit)
        .map { |r| reminder_line(r) }

      task_lines = Task
        .where(workspace: workspace, status: %i[suggested todo in_progress blocked])
        .where.not(due_at: nil)
        .where(due_at: window)
        .order(:due_at).limit(limit)
        .map { |t| task_line(t) }

      event_lines = CalendarEvent
        .joins(calendar: :calendar_account)
        .where(calendar_accounts: { workspace_id: workspace.id })
        .visible
        .where(start_at: window)
        .order(:start_at).limit(limit)
        .map { |e| event_line(e) }

      reminder_lines + task_lines + event_lines
    end

    def task_line(task)
      # - [task] Submit tax form — due 2026-07-30
      due = task.due_at ? " — due #{task.due_at.to_date.iso8601}" : ""
      "- [task] #{task.title}#{due}"
    end

    def reminder_line(reminder)
      # - [reminder/payment_due] Pay invoice #123 — 2026-07-30 (EUR 450.00)
      amount_part = if reminder.amount_cents.present?
        currency = reminder.currency.presence || "EUR"
        " (#{format('%s %.2f', currency, reminder.amount_cents / 100.0)})"
      else
        ""
      end
      "- [reminder/#{reminder.reminder_type}] #{reminder.title} — #{reminder.due_at.to_date.iso8601}#{amount_part}"
    end

    def event_line(event)
      # - [calendar event] Flight to Paris — 2026-08-02 10:05
      time_part = event.all_day? ? "" : " #{event.start_at.strftime('%H:%M')}"
      "- [calendar event] #{event.title} — #{event.start_at.to_date.iso8601}#{time_part}"
    end
  end
end
