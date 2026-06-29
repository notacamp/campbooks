module Discussions
  # Posts a system-authored "Scout" message into an email's discussion thread and
  # broadcasts it to live viewers. Used when Scout extracts something from an email
  # worth surfacing in the conversation — a calendar event or a reminder — with a
  # link back to it, so the discussion becomes the running record of what Scout did.
  #
  # Mirrors the lazy thread creation in DiscussionThreadable#find_or_create_agent_thread
  # and the broadcast in EmailChatReplyJob#broadcast_reply, but runs outside a request
  # (no Current.user): the mailbox owner is resolved explicitly and used both as the
  # message author (AgentMessage#user is NOT NULL) and for the locale the body renders
  # in. The body is built by the caller's block, evaluated inside the owner's locale,
  # and stored as Markdown (AI messages render Markdown with safe links).
  #
  # Best-effort: never raises into its caller. Returns the created AgentMessage, or
  # nil when it can't post (no email/thread/owner, a blank body, or create_if_missing:
  # false and no discussion exists yet).
  class ScoutAnnouncer
    def self.announce(email_message:, create_if_missing: true, &body)
      new(email_message: email_message, create_if_missing: create_if_missing).announce(&body)
    end

    def initialize(email_message:, create_if_missing: true)
      @email_message = email_message
      @create_if_missing = create_if_missing
    end

    def announce(&body)
      return nil unless @email_message

      email_thread = @email_message.email_thread
      return nil unless email_thread

      owner = owner_user(email_thread)
      return nil unless owner

      agent_thread = email_thread.agent_thread
      return nil if agent_thread.nil? && !@create_if_missing

      content = nil
      I18n.with_locale(owner.locale.presence || I18n.default_locale) { content = body.call(owner) }
      return nil if content.blank?

      agent_thread ||= create_agent_thread(email_thread, owner)
      message = agent_thread.agent_messages.create!(content: content, author_type: :ai, user: owner)

      broadcast(email_thread, message)
      message
    rescue => e
      Rails.logger.error("[Discussions::ScoutAnnouncer] failed for email_message=#{@email_message&.id}: #{e.message}")
      nil
    end

    private

    # The mailbox owner — the discussion thread's owning user, used as the AI
    # message's (required) author and as the locale for the body.
    def owner_user(email_thread)
      account = @email_message.email_account || email_thread.email_account
      account&.email_account_users&.find_by(owner: true)&.user
    end

    def create_agent_thread(email_thread, owner)
      email_thread.create_agent_thread!(
        title: email_thread.subject.presence || @email_message.subject.presence || email_thread.display_subject,
        purpose: :email_chat,
        user: owner,
        workspace: owner.workspace
      )
    end

    # Live-update every open viewer of the thread: drop the empty-state placeholder
    # (only present when the discussion had no messages) and append the new comment,
    # exactly as EmailChatReplyJob#broadcast_reply does for an @scout reply.
    def broadcast(email_thread, message)
      Turbo::StreamsChannel.broadcast_remove_to(email_thread, target: "discussion_empty")
      Turbo::StreamsChannel.broadcast_append_to(
        email_thread,
        target: "comments_list",
        partial: "email_comments/comment",
        locals: { comment: message, email_message: @email_message }
      )
    rescue => e
      Rails.logger.warn("[Discussions::ScoutAnnouncer] broadcast failed for message=#{message.id}: #{e.message}")
    end
  end
end
