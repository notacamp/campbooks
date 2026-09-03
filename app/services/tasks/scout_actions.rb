module Tasks
  # The actions Scout may run from a task discussion (Tasks::ChatReplyJob) — a
  # small task-side sibling of the EmailActions registry with the same result
  # shape ({ success:, tool:, message:, result: }). Scoped to what the task UI
  # itself offers: set the due date, and promote it into the task's single
  # deadline Reminder exactly like TasksController#remind (re-running syncs the
  # date instead of duplicating).
  module ScoutActions
    TOOLS = %w[set_due_date set_reminder].freeze

    # Everything here is reversible (a due date edit / a pending reminder), so
    # the whole set is safe to auto-execute from chat without confirmation.
    def self.auto_safe?(tool)
      TOOLS.include?(tool.to_s)
    end

    def self.run(tool, task:, args: {})
      args = (args || {}).stringify_keys
      case tool.to_s
      when "set_due_date" then set_due_date(task, args)
      when "set_reminder" then set_reminder(task, args)
      else failure(tool, I18n.t("tasks.scout_actions.unknown"))
      end
    rescue => e
      Rails.logger.error("[Tasks::ScoutActions] #{tool} failed for task #{task.id}: #{e.message}")
      failure(tool, I18n.t("tasks.scout_actions.error"))
    end

    def self.set_due_date(task, args)
      due_at, all_day = parse_due(args["due_at"])
      return failure("set_due_date", I18n.t("tasks.scout_actions.invalid_date")) unless due_at

      task.update!(due_at: due_at, all_day: all_day)
      success("set_due_date",
              I18n.t("tasks.scout_actions.set_due_date.success", due: I18n.l(due_at.to_date, format: :long)),
              { due_at: due_at.iso8601 })
    end

    def self.set_reminder(task, args)
      if args["due_at"].present?
        due_at, all_day = parse_due(args["due_at"])
        return failure("set_reminder", I18n.t("tasks.scout_actions.invalid_date")) unless due_at

        task.update!(due_at: due_at, all_day: all_day)
      end
      return failure("set_reminder", I18n.t("tasks.scout_actions.set_reminder.no_due")) if task.due_at.blank?

      reminder = task.reminders.find_or_initialize_by(reminder_type: :deadline)
      reminder.assign_attributes(
        workspace: task.workspace, title: task.title.to_s.first(255),
        due_at: task.due_at, all_day: task.all_day, confidence: 1.0
      )
      reminder.status = :pending if reminder.new_record?
      reminder.save!
      success("set_reminder",
              I18n.t("tasks.scout_actions.set_reminder.success", due: I18n.l(task.due_at.to_date, format: :long)),
              { reminder_id: reminder.id })
    end

    # Accepts "YYYY-MM-DD" (an all-day date) or a full ISO8601 datetime.
    # Returns [time, all_day] or nil when unparseable.
    def self.parse_due(raw)
      str = raw.to_s.strip
      return nil if str.blank?

      if /\A\d{4}-\d{2}-\d{2}\z/.match?(str)
        [ Date.iso8601(str).in_time_zone, true ]
      else
        time = Time.zone.parse(str)
        time ? [ time, false ] : nil
      end
    rescue ArgumentError
      nil
    end

    def self.success(tool, message, result = nil)
      { success: true, tool: tool, message: message, result: result }
    end

    def self.failure(tool, message)
      { success: false, tool: tool, message: message, result: nil }
    end
  end
end
