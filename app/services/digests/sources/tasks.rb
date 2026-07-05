# frozen_string_literal: true

module Digests
  module Sources
    # Gathers Tasks due in the lookahead window (and optionally overdue ones).
    # Scoped to the user's workspace via Task.accessible_to(user).
    class Tasks < Base
      def self.direction = :lookahead

      def items(period)
        include_overdue = source_config["include_overdue"]

        # The generator already applied window_days when computing the period.
        # Anchor on the period (not Time.current like due_soon) so job retries
        # cover the same window as the original run.
        scope = Task.accessible_to(user)
        due_scope     = scope.active.where(due_at: period.begin..period.end)
        overdue_scope = scope.overdue if include_overdue

        tasks = if overdue_scope
          Task.where(id: due_scope.select(:id))
              .or(Task.where(id: overdue_scope.select(:id)))
              .order(Arel.sql("CASE WHEN due_at IS NULL THEN 1 ELSE 0 END, due_at ASC"))
              .limit(MAX_ITEMS)
        else
          due_scope.order(due_at: :asc).limit(MAX_ITEMS)
        end

        tasks.map do |task|
          Digests::Item.new(
            source_type: "task",
            source_id:   task.id,
            title:       task.title,
            subtitle:    task_subtitle(task),
            summary:     truncate(task.description.to_s),
            timestamp:   task.due_at&.iso8601
          )
        end
      end

      private

      def task_subtitle(task)
        parts = []
        parts << I18n.l(task.due_at, format: :short) if task.due_at.present?
        if task.priority_high? || task.priority_urgent?
          priority_label = I18n.t(
            "activerecord.attributes.task.priorities.#{task.priority}",
            default: task.priority.to_s.humanize
          )
          parts << priority_label
        end
        parts.join(" · ")
      end
    end
  end
end
