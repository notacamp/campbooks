module Feed
  module Sources
    # Tasks on the feed, in two flavors:
    #
    # SUGGESTIONS — fresh, confident AI-extracted tasks awaiting triage, carrying
    # Accept/Dismiss right on the card (mirrors Sources::Reminder's pending cards).
    # Their own dedupe_key ("task_suggestion:") so accepting one doesn't block the
    # accepted task from surfacing later as an attention card — acted state is
    # preserved per dedupe_key across refreshes.
    #
    # ACTIVE tasks needing attention — assigned to the user, OR due within the
    # horizon / overdue, OR blocked. Done and cancelled tasks are out. Ranked by
    # urgency (overdue first); suggestions slot between due-soon and someday.
    class Task < Feed::Source
      HORIZON = 14.days
      ATTENTION_WITHIN = 1.day
      # Suggestions below this confidence stay on the tasks triage page only.
      SUGGESTION_MIN_CONFIDENCE = 0.6
      # Older untriaged suggestions stop crowding the feed; they stay triageable
      # from the tasks page.
      SUGGESTION_WINDOW = 14.days

      def self.key = "task"

      def candidates
        active_candidates + suggestion_candidates
      end

      def still_valid?(item, task)
        return false if task.nil? || task.archived?

        suggestion_item?(item) ? task.suggested? : task.active?
      end

      private

      def active_candidates
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

      def suggestion_candidates
        suggestions = ::Task.accessible_to(user).triage
          .where(confidence: SUGGESTION_MIN_CONFIDENCE..)
          .where(created_at: (now - SUGGESTION_WINDOW)..)

        suggestions.map do |task|
          {
            subject: task,
            dedupe_key: "task_suggestion:#{task.id}",
            sort_at: task.created_at,
            score: 55,
            attention: false,
            data: { "status" => task.status, "due_at" => task.due_at&.iso8601 }
          }
        end
      end

      def suggestion_item?(item)
        item.dedupe_key.to_s.start_with?("task_suggestion:")
      end

      def attention?(task)
        task.blocked? || (task.due_at.present? && task.due_at <= now + ATTENTION_WITHIN)
      end

      # Blocked is a state, not a date — flat. Dated tasks glide: 92 overdue,
      # 78 at one day out, 48 at a week, 22 at the horizon's edge.
      def score_for(task)
        return 85 if task.blocked?

        if task.due_at
          days = (task.due_at - now) / 1.day
          return 92 if days <= 0 # overdue

          if days <= 1
            ramp(days, from: 0, to: 1, at_from: 92, at_to: 78)
          elsif days <= 7
            ramp(days, from: 1, to: 7, at_from: 78, at_to: 48)
          else
            ramp(days, from: 7, to: 14, at_from: 48, at_to: 22)
          end
        else
          20 # assigned to me, no due date
        end
      end
    end
  end
end
