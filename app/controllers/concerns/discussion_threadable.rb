# Shared helpers for the per-email discussion thread (comments + follows).
module DiscussionThreadable
  extend ActiveSupport::Concern

  private

  # The email message, scoped to what the current user may access: either mailbox
  # read access, or being a follower of its discussion thread (pulled in by an
  # @mention). Raises RecordNotFound otherwise — we don't leak existence.
  def accessible_email_message(id)
    message = EmailMessage.find(id)
    return message if message.email_account&.accessible_by?(Current.user)

    agent_thread = message.email_thread&.agent_thread
    return message if agent_thread && ThreadFollow.exists?(user: Current.user, agent_thread: agent_thread)

    raise ActiveRecord::RecordNotFound, "EmailMessage #{id} is not accessible"
  end

  # Find-or-create the AgentThread backing an email's discussion. Both the
  # EmailThread and its AgentThread are created lazily — an email has neither
  # until the first comment or follow.
  def find_or_create_agent_thread(email_message)
    email_thread = email_message.email_thread || create_email_thread(email_message)

    email_thread.agent_thread || email_thread.create_agent_thread!(
      title: email_thread.subject,
      purpose: :email_chat,
      user: Current.user,
      workspace: Current.user.workspace
    )
  end

  def create_email_thread(email_message)
    Emails::Threading.find_or_create(email_message)
                     .tap { |thread| email_message.update!(email_thread: thread) }
  end
end
