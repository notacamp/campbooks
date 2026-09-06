module Feed
  module Sources
    # Asks on the feed. An ask (a Task row) earns a card of its own only when it
    # needs a decision from the reader — otherwise it rides its source email's card
    # (Campbooks::Feed::EmailActionCard) or lives quietly on Time. Three flavours,
    # all drawn from the live set (so snoozed asks never appear; a lapsed snooze
    # brings the ask back through the normal rules):
    #
    #   suggested — a fresh, confident AI-found ask awaiting a first decision, UNLESS
    #               its source email still has an active card (then it rides that).
    #   undated   — an accepted ask with no date: it needs a "when".
    #   due       — an accepted ask due today or overdue: it needs doing now.
    #
    # No other dated ask gets a card — future-dated ones live on Time. `framing` is
    # stamped into data so the card doesn't recompute it.
    class Task < Feed::Source
      # Suggestions below this confidence stay off the feed (triageable on Time).
      SUGGESTION_MIN_CONFIDENCE = 0.6
      # Older untriaged suggestions stop crowding the feed.
      SUGGESTION_WINDOW = 14.days

      def self.key = "task"

      def candidates
        suggestion_candidates + active_candidates
      end

      def still_valid?(item, task)
        return false if task.nil? || task.archived? || task.snoozed?

        if suggestion_item?(item)
          task.suggested?
        else
          task.active? && (task.due_at.nil? || task.due_at <= end_of_today)
        end
      end

      private

      # 1. Suggested asks (a first decision), minus the ones whose source email still
      #    has an active card — the ask rides that card, so a second one would double up.
      def suggestion_candidates
        suggestions = ::Task.accessible_to(user).live.where(status: :suggested)
          .where(confidence: SUGGESTION_MIN_CONFIDENCE..)
          .where(created_at: (now - SUGGESTION_WINDOW)..)
          .includes(:source)

        carded = threads_with_active_email_card
        suggestions.reject { |task| rides_email_card?(task, carded) }.map do |task|
          {
            subject: task,
            dedupe_key: "task_suggestion:#{task.id}",
            sort_at: task.created_at,
            score: 55,
            attention: false,
            data: framing_data(task, "suggested")
          }
        end
      end

      # 2 + 3. Accepted asks that need a decision: undated (needs a "when"), or due
      #        today / overdue (needs doing). Future-dated ones are Time-only.
      def active_candidates
        base = ::Task.accessible_to(user).live.where(status: ::Task::ACTIVE_STATUSES).includes(:source)

        undated = base.where(due_at: nil).map do |task|
          {
            subject: task, dedupe_key: "task:#{task.id}", sort_at: task.updated_at,
            score: 40, attention: false, data: framing_data(task, "undated")
          }
        end

        due = base.where.not(due_at: nil).where(due_at: ..end_of_today).map do |task|
          overdue = task.due_at < now
          {
            subject: task, dedupe_key: "task:#{task.id}", sort_at: task.due_at,
            score: overdue ? 92 : 88, attention: true, data: framing_data(task, "due")
          }
        end

        undated + due
      end

      def framing_data(task, framing)
        { "status" => task.status, "due_at" => task.due_at&.iso8601, "framing" => framing }
      end

      def suggestion_item?(item)
        item.dedupe_key.to_s.start_with?("task_suggestion:")
      end

      def rides_email_card?(task, carded)
        email = task.source_email
        return false unless email

        carded.include?(email.email_thread_id || email.id)
      end

      # The thread keys (email_thread_id, or the message id when threadless) that
      # currently carry an active email_action card for this user — one grouped query.
      def threads_with_active_email_card
        message_ids = user.feed_items.active
          .where(kind: "email_action", subject_type: "EmailMessage")
          .pluck(:subject_id)
        return Set.new if message_ids.empty?

        EmailMessage.where(id: message_ids).pluck(:id, :email_thread_id)
                    .map { |id, thread_id| thread_id || id }.to_set
      end

      def end_of_today
        @end_of_today ||= now.in_time_zone(user.effective_time_zone).end_of_day
      end
    end
  end
end
