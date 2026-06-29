module Tasks
  # Scout's reply in a task discussion. Mirrors EmailChatReplyJob but leaner: no
  # auto-actions (tasks have no inbox tools) and no record write-back. Replies only
  # when the triggering comment @scout-tagged Scout; broadcasts to the Task stream.
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

      reply = thread.agent_messages.create!(
        content: result[:reply],
        author_type: :ai,
        ai_suggested_actions: [],
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
