class ComposeChatReplyJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 2

  def perform(agent_message_id)
    message = AgentMessage.find(agent_message_id)
    return unless message.from_user?

    thread = message.agent_thread
    return unless thread

    # Establish acting identity for AI config/prompt lookups and any scoped reads.
    Current.acting_user = thread.user
    Current.workspace = thread.workspace

    return if thread.agent_messages.where(author_type: :ai).where("created_at > ?", message.created_at).exists?

    result = Ai::ComposeChatService.new(thread).reply_to(message)

    if result.blank? || result[:reply].blank?
      I18n.with_locale(thread.user.locale.presence || I18n.default_locale) do
        thread.agent_messages.create!(
          content: I18n.t("jobs.compose_chat_reply.error"),
          author_type: :ai,
          ai_suggested_actions: [],
          user: thread.user
        )
      end
    else
      thread.agent_messages.create!(
        content: result[:reply],
        author_type: :ai,
        ai_suggested_actions: result[:suggested_actions] || [],
        ai_auto_actions: result[:auto_actions] || [],
        user: thread.user
      )
    end

    broadcast_update(thread, result[:auto_actions] || [])
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[ComposeChatReplyJob] Message #{agent_message_id} not found, skipping")
  rescue => e
    Rails.logger.error("[ComposeChatReplyJob] Error: #{e.message}")
    raise
  end

  private

  def broadcast_update(thread, auto_actions = [])
    messages = thread.agent_messages.chronological.last(50)
    html = ApplicationController.render(
      partial: "email_compose_chat/messages_panel",
      locals: { thread: thread, messages: messages }
    )
    # Build script tags for auto_actions — Turbo evaluates <script> in stream templates
    auto_scripts = auto_actions.map { |a| "<script data-auto-action>var c=Stimulus.getControllerForElementAndIdentifier(document.querySelector('[data-controller~=compose-chat]'),'compose-chat');if(c)c.__executeAutoAction__('#{a['tool']}',#{a['args'].to_json})</script>" }.join
    wrapped = "<div id=\"compose_messages_wrapper\" class=\"flex flex-col flex-1 min-h-0\">#{html}#{auto_scripts}</div>"
    Turbo::StreamsChannel.broadcast_replace_to(
      "compose_chat_#{thread.user.id}",
      target: "compose_messages_wrapper",
      html: wrapped
    )
  end
end
