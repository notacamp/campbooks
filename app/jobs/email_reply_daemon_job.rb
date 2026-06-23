class EmailReplyDaemonJob < ApplicationJob
  queue_as :default

  def perform
    candidate_ids = AgentMessage.where(author_type: :user, reply_status: :pending)
                                .joins(:agent_thread)
                                .where(agent_threads: { purpose: :email_chat })
                                .order(created_at: :asc)
                                .limit(10)
                                .pluck(:id)

    return if candidate_ids.empty?

    AgentMessage.where(id: candidate_ids, reply_status: :pending)
                .update_all(reply_status: :processing, updated_at: Time.current)

    AgentMessage.where(id: candidate_ids, reply_status: :processing)
                .order(created_at: :asc)
                .each do |message|
      process_message(message)
    rescue => e
      Rails.logger.error("[EmailReplyDaemon] Error processing message #{message.id}: #{e.message}")
      message.update_column(:reply_status, :failed)
    end

    AgentMessage.where(reply_status: :processing)
                .where("updated_at < ?", 5.minutes.ago)
                .update_all(reply_status: :pending)
  end

  private

  def process_message(message)
    agent_thread = message.agent_thread
    email_thread = agent_thread&.contextable
    return unless email_thread.is_a?(EmailThread)

    # This job batches messages from different threads/users, so set the acting
    # identity per message (CurrentAttributes only reset per job, not per loop).
    Current.acting_user = agent_thread.user
    Current.workspace = agent_thread.workspace

    return if agent_thread.agent_messages.where(author_type: :ai).where("created_at > ?", message.created_at).exists?

    agent_thread.agent_messages.where(draft: true, outdated: false).update_all(outdated: true)

    result = Ai::ChatService.reply_to(message)
    return unless result && result[:reply].present?

    auto_results = execute_auto_actions(email_thread.latest_message, result[:auto_actions] || [])

    reply_content = result[:reply]
    failures = auto_results.reject { |r| r[:success] }
    if failures.any?
      prefix = I18n.t("jobs.email_reply_daemon.auto_action_failures_prefix",
                       locale: agent_thread.user.locale.presence || I18n.default_locale)
      reply_content += "\n\n#{prefix}\n#{failures.map { |f| "- #{f[:message]}" }.join("\n")}"
    end

    reply = agent_thread.agent_messages.create!(
      content: reply_content,
      author_type: :ai,
      ai_suggested_actions: result[:suggested_actions] || [],
      ai_auto_actions: auto_results.map { |r| { "tool" => r[:tool], "message" => r[:message], "success" => r[:success] } },
      user: agent_thread.user
    )

    message.update_column(:reply_status, :replied)

    latest_message = email_thread.latest_message
    if latest_message && result[:suggested_actions].any?
      action_text = reply.content.truncate(140)
      latest_message.update_columns(
        ai_action_prompt: action_text,
        ai_suggested_actions: result[:suggested_actions],
        ai_analysis_message_id: reply.id
      )
    end

    broadcast_reply(reply, auto_results)
  end

  def execute_auto_actions(email_message, auto_actions)
    auto_actions.map do |action|
      Tools::Executor.call(
        tool: action["tool"],
        email_message: email_message,
        args: action["args"] || {}
      )
    end
  end

  def broadcast_reply(reply, auto_results = [])
    agent_thread = reply.agent_thread
    email_thread = agent_thread.contextable
    email_message = email_thread&.latest_message

    stream = email_thread

    Turbo::StreamsChannel.broadcast_remove_to(stream, target: "scout_typing")
    Turbo::StreamsChannel.broadcast_append_to(
      stream,
      target: "comments_list",
      partial: "email_comments/comment",
      locals: { comment: reply, email_message: email_message }
    )

    auto_results.each do |r|
      next unless r[:success]

      case r[:tool]
      when "add_tag", "remove_tag"
        thread_tags_html = ApplicationController.render(
          partial: "email_messages/thread_tags",
          locals: { message: email_message }
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          stream,
          target: ActionView::RecordIdentifier.dom_id(email_message, :thread_tags),
          html: thread_tags_html
        )
        tags_html = ApplicationController.render(
          partial: "email_messages/tags",
          locals: { message: email_message }
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          stream,
          target: ActionView::RecordIdentifier.dom_id(email_message, :tags),
          html: tags_html
        )
      when "archive", "trash"
        Turbo::StreamsChannel.broadcast_remove_to(stream, target: "email_todo_#{email_message.id}")
        Turbo::StreamsChannel.broadcast_remove_to(
          stream,
          target: ActionView::RecordIdentifier.dom_id(email_message.email_thread, :thread_item)
        )
        archived_msg = I18n.t("jobs.email_reply_daemon.conversation_archived",
                               locale: agent_thread.user.locale.presence || I18n.default_locale)
        detail_html = ApplicationController.render(
          partial: "email_messages/empty_detail",
          locals: { message: archived_msg }
        )
        Turbo::StreamsChannel.broadcast_replace_to(stream, target: "email_content", html: detail_html)
        thread_tags_html = ApplicationController.render(
          partial: "email_messages/thread_tags",
          locals: { message: email_message }
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          stream,
          target: ActionView::RecordIdentifier.dom_id(email_message, :thread_tags),
          html: thread_tags_html
        )
      end
    end

    auto_results.each do |r|
      html = ApplicationController.render(
        partial: "shared/action_toast",
        locals: { message: r[:message], variant: r[:success] ? :success : :error }
      )
      Turbo::StreamsChannel.broadcast_append_to(stream, target: "action_toasts", html: html)
    end
  end
end
