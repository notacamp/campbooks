module Tasks
  # Scout's reply in a task discussion. Mirrors EmailChatReplyJob but leaner: the
  # only tools are the task-side Tasks::ScoutActions (due date + deadline reminder)
  # and there is no record write-back. Replies only when the triggering comment
  # @scout-tagged Scout; broadcasts to the Task stream.
  class ChatReplyJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 2

    def perform(agent_message_id)
      message = AgentMessage.find(agent_message_id)
      return unless message.from_user? && message.mentions_scout?

      claimed = AgentMessage.where(id: agent_message_id, reply_status: :pending)
                            .update_all(reply_status: :processing)
      return if claimed.zero?

      thread = message.agent_thread
      task = thread&.contextable
      return unless task.is_a?(Task)

      # No session cookie in a background job — set the acting identity from the thread.
      Current.acting_user = thread.user
      Current.workspace = thread.workspace

      return if thread.agent_messages.where(author_type: :ai).where("created_at > ?", message.created_at).exists?

      result = Ai::ChatService.reply_to(message)
      if result.blank? || result[:reply].blank?
        message.update_column(:reply_status, :failed)
        broadcast_failure(task)
        return
      end

      auto_results = execute_auto_actions(task, result[:auto_actions] || [])

      reply_content = result[:reply]
      failures = auto_results.reject { |r| r[:success] }
      if failures.any?
        prefix = I18n.t("jobs.task_chat_reply.auto_action_failures_prefix",
                        locale: thread.user&.locale.presence || I18n.default_locale)
        reply_content += "\n\n#{prefix}\n#{failures.map { |f| "- #{f[:message]}" }.join("\n")}"
      end

      reply = thread.agent_messages.create!(
        content: reply_content,
        author_type: :ai,
        ai_suggested_actions: [],
        ai_auto_actions: auto_results.map { |r| { "tool" => r[:tool], "message" => r[:message], "success" => r[:success] } },
        ai_provenance: result[:provenance] || {},
        user: thread.user
      )
      message.update_column(:reply_status, :replied)
      broadcast_reply(task, reply)
    rescue ActiveRecord::RecordNotFound
      Rails.logger.warn("[Tasks::ChatReplyJob] message #{agent_message_id} not found, skipping")
    rescue => e
      Rails.logger.error("[Tasks::ChatReplyJob] Error: #{e.message}")
      AgentMessage.where(id: agent_message_id).update_all(reply_status: :pending)
      raise
    end

    private

    # Model output is untrusted — only whitelisted, reversible task actions run
    # unconfirmed (mirrors EmailChatReplyJob#execute_auto_actions).
    def execute_auto_actions(task, auto_actions)
      auto_actions.filter_map do |action|
        next unless action.is_a?(Hash)

        action = action.stringify_keys
        tool = action["tool"].to_s
        unless Tasks::ScoutActions.auto_safe?(tool)
          Rails.logger.warn("[Tasks::ChatReplyJob] Blocked auto-execution of unknown action '#{tool}'")
          next
        end

        Tasks::ScoutActions.run(tool, task: task, args: action["args"] || {})
      end
    end

    def broadcast_reply(task, reply)
      Turbo::StreamsChannel.broadcast_remove_to(task, target: "scout_typing")
      Turbo::StreamsChannel.broadcast_append_to(
        task, target: "comments_list",
        partial: "tasks/comments/comment", locals: { comment: reply, task: task }
      )
    rescue => e
      Rails.logger.warn("[Tasks::ChatReplyJob] reply broadcast failed: #{e.message}")
    end

    def broadcast_failure(task)
      Turbo::StreamsChannel.broadcast_remove_to(task, target: "scout_typing")
      Turbo::StreamsChannel.broadcast_append_to(task, target: "comments_list", partial: "tasks/comments/error")
    rescue => e
      Rails.logger.warn("[Tasks::ChatReplyJob] failure broadcast failed: #{e.message}")
    end
  end
end
