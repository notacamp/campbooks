class AgentChatReplyJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 2

  def perform(agent_message_id)
    message = AgentMessage.find(agent_message_id)
    return unless message.from_user?

    thread = message.agent_thread
    return unless thread

    # Establish the acting identity for the whole tool loop: Scout runs in a
    # background job with no session cookie, so set it explicitly here. Every
    # tool then gates on Current.user, scoped to this user's permitted accounts.
    Current.acting_user = thread.user
    Current.workspace = thread.workspace

    # Prevent duplicate replies on retry
    return if thread.agent_messages.where(author_type: :ai).where("created_at > ?", message.created_at).exists?

    # Stream live status into the typing indicator while Scout works its tools.
    on_status = ->(label) { broadcast_typing_status(thread, label) }

    result = compute_reply(thread, message, on_status)
    return broadcast_error(thread, message) if result.blank? || result[:reply].blank?

    # Execute auto_actions server-side
    auto_results = execute_auto_actions(result[:auto_actions] || [])

    # Append failure info to the reply if any auto_actions failed
    reply_content = result[:reply]
    failures = auto_results.reject { |r| r[:success] }
    if failures.any?
      failure_text = failures.map { |f| "- #{f[:message]}" }.join("\n")
      reply_content += "\n\nHowever, I couldn't complete these actions:\n#{failure_text}"
    end

    if thread.title == "New chat" || thread.title == "Default chat"
      title = result[:title].presence || message.content.truncate(60)
      thread.update!(title: title)
    end

    ai_message = thread.agent_messages.create!(
      content: reply_content,
      author_type: :ai,
      ai_suggested_actions: result[:suggested_actions] || [],
      ai_auto_actions: auto_results.map { |r| { "tool" => r[:tool], "message" => r[:message], "success" => r[:success] } },
      ai_prompts: result[:prompts] || [],
      ai_provenance: result[:provenance] || {},
      ai_thinking: result[:thinking],
      steps: result[:steps] || [],
      user: thread.user,
      reply_status: :replied
    )

    broadcast_reply(thread, ai_message)
    message.replied!

    # Persistent bell entry if the reply came back slowly (you likely navigated away)
    Notifier.scout_reply(thread, prompt_at: message.created_at, link_url: "/scout/threads/#{thread.id}")

    # Broadcast toasts for auto-executed actions
    auto_results.each do |r|
      html = ApplicationController.render(
        partial: "shared/action_toast",
        locals: { message: r[:message], variant: r[:success] ? :success : :error }
      )
      Turbo::StreamsChannel.broadcast_append_to(
        "agent_chat_#{thread.user.id}",
        target: "action_toasts",
        html: html
      )
    end
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[AgentChatReplyJob] Message #{agent_message_id} not found, skipping")
  rescue => e
    Rails.logger.error("[AgentChatReplyJob] Error (attempt #{executions}): #{e.message}")
    # Let retry_on handle earlier attempts; on the final one, tell the user
    # instead of leaving the typing indicator spinning forever.
    raise if executions < 2
    broadcast_error(thread, message) if thread && message
  end

  private

  # Global Scout runs the native-tool-calling agent; email/compose threads keep
  # their existing services. Returns a uniform hash (always includes :thinking
  # and :steps; un-migrated paths simply leave them nil/empty).
  def compute_reply(thread, message, on_status)
    return Ai::ChatService.reply_to(message, on_status: on_status) unless thread.global?

    on_event = ->(type, label) { broadcast_typing_status(thread, agent_status(type, label)) }
    result = Scout::Agent.new(thread, on_event: on_event).run(message.content)
    return nil unless result

    {
      reply: result.reply, thinking: result.thinking, steps: result.steps,
      suggested_actions: result.suggested_actions, prompts: result.prompts,
      provenance: result.provenance, auto_actions: []
    }
  end

  # Map an agent event to a human typing-indicator label.
  def agent_status(type, label)
    case type
    when :tool     then I18n.t("scout.status.#{label}", default: I18n.t("scout.status.working", default: "Working…"))
    when :proposal then I18n.t("scout.status.preparing", default: "Preparing an action…")
    else label.presence || I18n.t("scout.status.working", default: "Working…")
    end
  end

  def execute_auto_actions(auto_actions)
    auto_actions.filter_map do |action|
      tool = action["tool"].to_s
      unless EmailActions.auto_safe?(tool)
        Rails.logger.warn("[AgentChatReplyJob] Blocked auto-execution of unsafe action '#{tool}' — must be confirmed by the user")
        next
      end

      Tools::Executor.call(
        tool: tool,
        args: action["args"] || {}
      )
    end
  end

  # Swap the typing indicator for Scout's reply: drop the dots and append the
  # message (marked latest so its follow-up prompts render). Appending — rather
  # than replacing the whole panel — keeps the conversation flicker-free and lets
  # only the new message animate in.
  def broadcast_reply(thread, ai_message)
    stream = "agent_chat_#{thread.user.id}"
    Turbo::StreamsChannel.broadcast_remove_to(stream, target: "agent_typing")
    Turbo::StreamsChannel.broadcast_append_to(
      stream,
      target: "agent_messages_list",
      partial: "agent_chat/message",
      locals: { agent_message: ai_message, latest: true }
    )
  end

  # Reply failed or came back empty: swap the typing dots for a calm, retryable
  # notice so the user is never left staring at a stalled indicator. Persist the
  # failure too, so a page reload renders the error card instead of a phantom
  # "Thinking…" — the panel keys the spinner off the user message's pending state.
  def broadcast_error(thread, user_message)
    stream = "agent_chat_#{thread.user.id}"
    Turbo::StreamsChannel.broadcast_remove_to(stream, target: "agent_typing")
    Turbo::StreamsChannel.broadcast_append_to(
      stream,
      target: "agent_messages_list",
      partial: "agent_chat/error",
      locals: { retry_content: user_message.content }
    )
    user_message.failed! if user_message&.persisted? && !user_message.failed?
  rescue => e
    Rails.logger.warn("[AgentChatReplyJob] error broadcast failed: #{e.message}")
  end

  def broadcast_typing_status(thread, label)
    Turbo::StreamsChannel.broadcast_replace_to(
      "agent_chat_#{thread.user.id}",
      target: "agent_typing",
      partial: "agent_chat/typing",
      locals: { status: label }
    )
  rescue => e
    Rails.logger.warn("[AgentChatReplyJob] typing status broadcast failed: #{e.message}")
  end
end
