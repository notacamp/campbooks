module Feed
  module Sources
    # Tasks that need attention: assigned to the user, OR due within the horizon /
    # overdue, OR blocked. Suggested (AI-proposed) tasks live in Skim, not the feed;
    # done and cancelled tasks are out. Ranked by urgency (overdue first).
    class Task < Feed::Source
      HORIZON = 14.days
      ATTENTION_WITHIN = 1.day

      def self.key = "task"

      def candidates
        # `::Task` — disambiguate the model from this source class.
        relevant = ::Task.accessible_to(user).active.includes(:assignees, :tags)
          .left_joins(:task_assignments)
          .where(
            "task_assignments.user_id = :uid OR tasks.status = :blocked OR (tasks.due_at IS NOT NULL AND tasks.due_at <= :horizon)",
            uid: user.id, blocked: ::Task.statuses[:blocked], horizon: now + HORIZON
          )
          .distinct

        relevant.map do |task|
          {
            subject: task,
            dedupe_key: "task:#{task.id}",
            sort_at: task.due_at || task.updated_at,
            score: score_for(task),
            attention: attention?(task),
            data: { "status" => task.status, "due_at" => task.due_at&.iso8601 }
          }
        end
      end

      def still_valid?(_item, task)
        task.present? && task.active? && !task.archived?
      end

      private

      def attention?(task)
        task.blocked? || (task.due_at.present? && task.due_at <= now + ATTENTION_WITHIN)
      end

      def score_for(task)
        return 85 if task.blocked?

        if task.due_at
          days = ((task.due_at - now) / 1.day).floor
          return 90 if days <= 0    # overdue
          return 80 if days <= 1    # due today/tomorrow
          return 60 if days <= 7
          return 40
        end
        20    # assigned to me, no due date
      end
    end
  end
end
