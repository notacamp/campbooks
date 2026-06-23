# Drives one turn of the conversational setup assistant. Runs the AI call off
# the web request and streams the result back into the open dialog, mirroring
# AgentChatReplyJob's typing-indicator + Turbo Streams pattern.
class AiSetupChatReplyJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 2

  def perform(agent_message_id, kind)
    message = AgentMessage.find(agent_message_id)
    return unless message.from_user?

    thread = message.agent_thread
    return unless thread

    # AI config + workspace_context resolve through Current, which isn't set in a
    # background job — establish it from the thread (same as the other AI jobs).
    Current.acting_user = thread.user
    Current.workspace = thread.workspace

    # Prevent a duplicate reply if the job is retried after it already answered.
    return if thread.agent_messages.where(author_type: :ai).where("created_at > ?", message.created_at).exists?

    history = thread.agent_messages.chronological.map do |m|
      { role: m.from_user? ? "user" : "assistant", content: m.content }
    end

    result = Ai::OnboardingAssistant.new(thread.workspace).conversational_turn(history: history, kind: kind)

    I18n.with_locale(thread.user.locale.presence || I18n.default_locale) do
      case result[:type]
      when :question
        ai_message = thread.agent_messages.create!(
          content: result[:question], author_type: :ai, user: thread.user, reply_status: :replied
        )
        broadcast_message(thread, ai_message, hint: result[:hint])
      when :proposal
        if result[:items].blank?
          broadcast_error(thread, I18n.t("jobs.ai_setup_chat_reply.no_suggestions_yet"))
        else
          intro = I18n.t("jobs.ai_setup_chat_reply.proposal_intro", kind: kind.to_s.humanize.downcase)
          ai_message = thread.agent_messages.create!(
            content: intro, author_type: :ai, user: thread.user, reply_status: :replied,
            ai_suggested_actions: result[:items]
          )
          broadcast_message(thread, ai_message)
          broadcast_proposal(thread, result[:items], kind)
        end
      else
        broadcast_error(thread, error_text(result[:reason]))
      end
    end
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[AiSetupChatReplyJob] message #{agent_message_id} not found, skipping")
  rescue => e
    Rails.logger.error("[AiSetupChatReplyJob] error (attempt #{executions}): #{e.message}")
    raise if executions < 2
    I18n.with_locale(thread&.user&.locale.presence || I18n.default_locale) do
      broadcast_error(thread, I18n.t("jobs.ai_setup_chat_reply.error_retry"))
    end if thread
  end

  private

  def stream_name(thread)
    "ai_setup_chat_#{thread.user_id}"
  end

  # Drop the typing dots, then append the new chat turn.
  def broadcast_message(thread, message, hint: nil)
    Turbo::StreamsChannel.broadcast_remove_to(stream_name(thread), target: "setup_chat_typing_#{thread.id}")
    Turbo::StreamsChannel.broadcast_append_to(
      stream_name(thread),
      target: "setup_chat_messages_#{thread.id}",
      partial: "ai_setup_chats/message",
      locals: { agent_message: message, hint: hint }
    )
  end

  def broadcast_proposal(thread, items, kind)
    Turbo::StreamsChannel.broadcast_replace_to(
      stream_name(thread),
      target: "setup_chat_proposal_#{thread.id}",
      partial: "ai_setup_chats/proposal",
      locals: { thread: thread, items: items, kind: kind.to_s }
    )
  end

  def broadcast_error(thread, text)
    Turbo::StreamsChannel.broadcast_remove_to(stream_name(thread), target: "setup_chat_typing_#{thread.id}")
    Turbo::StreamsChannel.broadcast_append_to(
      stream_name(thread),
      target: "setup_chat_messages_#{thread.id}",
      partial: "ai_setup_chats/error",
      locals: { message: text }
    )
  rescue => e
    Rails.logger.warn("[AiSetupChatReplyJob] error broadcast failed: #{e.message}")
  end

  def error_text(reason)
    case reason
    when :no_ai_config
      I18n.t("jobs.ai_setup_chat_reply.error_no_ai_config")
    else
      I18n.t("jobs.ai_setup_chat_reply.error_generic")
    end
  end
end
