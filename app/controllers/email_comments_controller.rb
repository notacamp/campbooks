class EmailCommentsController < ApplicationController
  include DiscussionThreadable

  before_action :require_authentication

  def create
    email_message = accessible_email_message(params[:email_message_id])
    agent_thread = find_or_create_agent_thread(email_message)
    message = agent_thread.agent_messages.create!(
      content: params[:content],
      author_type: :user,
      user: Current.user,
      reply_status: :pending
    )

    process_participants(message, email_message)

    # The AI only joins in when explicitly tagged with @scout — otherwise this is
    # a plain comment between teammates, so no reply job and no typing indicator.
    streams = [
      turbo_stream.remove("discussion_empty"),
      turbo_stream.append("comments_list", partial: "email_comments/comment", locals: { comment: message, email_message: email_message }),
      turbo_stream.replace("comment_form", partial: "email_comments/form", locals: { email_message: email_message }),
      # Commenting auto-follows the thread — flip the header toggle to "Following".
      turbo_stream.replace("follow_toggle", partial: "email_messages/follow_toggle", locals: { email_message: email_message, following: true })
    ]

    if message.mentions_scout?
      if ai_provider_available?(:text)
        EmailChatReplyJob.perform_later(message.id)
        streams.insert(2, turbo_stream.append("comments_list", partial: "email_comments/typing"))
      else
        # No text provider: the comment still posts, but Scout can't reply. Mark it
        # terminal so the reply daemon doesn't keep retrying, and tell the user.
        message.update!(reply_status: :failed)
        streams << notify_stream(t("components.ai_setup_prompt.text.title"), severity: :warning)
      end
    end

    respond_to do |format|
      format.turbo_stream { render turbo_stream: streams }
      format.html { redirect_to email_message_path(email_message) }
    end
  end

  def poll
    email_message = accessible_email_message(params[:email_message_id])
    thread = email_message.email_thread
    return head :no_content unless thread

    agent_thread = thread.agent_thread
    return head :no_content unless agent_thread

    since = params[:since]&.to_i || 0
    since_time = Time.at(since / 1000.0) rescue 1.minute.ago

    new_comments = agent_thread.agent_messages.where(author_type: :ai)
      .where("created_at > ?", since_time)
      .order(created_at: :asc)

    if new_comments.any?
      render turbo_stream: [
        turbo_stream.remove("scout_typing"),
        *new_comments.map { |c| turbo_stream.append("comments_list", partial: "email_comments/comment", locals: { comment: c, email_message: email_message }) }
      ]
    else
      head :no_content
    end
  end

  private

  def process_participants(message, email_message)
    thread = message.agent_thread

    # Whoever comments follows the thread, so they hear about the discussion.
    ThreadFollow.find_or_create_by!(user: Current.user, agent_thread: thread)

    mentioned = mentioned_users(message.content)
    mentioned.each do |user|
      # An @mention pulls a teammate in: grants thread access (via the follow)
      # and notifies them directly.
      ThreadFollow.find_or_create_by!(user: user, agent_thread: thread)
      Notifier.thread_mention(
        thread: thread, comment: message, mentioned_user: user,
        actor: Current.user, email_message: email_message
      )
    end

    # Everyone else following the thread hears it as quiet activity. Mentioned
    # users already got the louder mention; you don't notify yourself.
    Notifier.thread_activity(
      thread: thread, comment: message, actor_name: Current.user.name,
      email_message: email_message, exclude: [ Current.user, *mentioned ]
    )
  end

  # Workspace teammates named with "@Full Name" (the autocomplete inserts full
  # names). Matching the whole name avoids first-name ambiguity, and (?<!\w)
  # keeps email addresses like foo@partner.com from counting as a mention.
  def mentioned_users(content)
    text = content.to_s
    Current.workspace.users.where.not(id: Current.user.id).select do |user|
      user.name.present? && text.match?(/(?<!\w)@#{Regexp.escape(user.name)}\b/i)
    end
  end
end
