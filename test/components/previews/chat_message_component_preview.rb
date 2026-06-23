# Preview for Campbooks::ChatMessage — the single message row used in Scout chat
# (:chat layout) and the per-email discussion thread (:comments layout).
class ChatMessageComponentPreview < ViewComponent::Preview
  # 1:1 assistant chat: the user's own messages align right as input pills,
  # Scout's reply renders as markdown prose on the canvas.
  def chat_layout
    render_thread(
      [
        user_msg("Can you summarize this email and flag anything urgent?"),
        ai_msg("Here's the gist: the vendor wants to push the deadline to next Friday. Nothing urgent, but they expect a reply this week.")
      ],
      layout: :chat
    )
  end

  # Multi-author discussion: every message is left-aligned with an author header,
  # Scout carries an "AI" badge so its authorship isn't signalled by colour alone.
  def comments_layout
    render_thread(
      [
        user_msg("Should we accept the new deadline?", name: "Alex Demo"),
        user_msg("Friday works for me. @scout what did they originally commit to?", name: "Partner"),
        ai_msg("The original statement of work committed to delivery by the 12th. Friday the 16th is a 4-day slip, and there's no penalty clause, so accepting it is low risk.")
      ],
      layout: :comments
    )
  end

  private

  def user_msg(content, name: "You")
    AgentMessage.new(content: content, author_type: :user, created_at: Time.current, user: User.new(name: name))
  end

  def ai_msg(content)
    AgentMessage.new(content: content, author_type: :ai, created_at: Time.current, ai_suggested_actions: [], ai_auto_actions: [])
  end

  def render_thread(messages, layout:)
    rows = messages.map do |m|
      render(Campbooks::ChatMessage.new(message: m, context: :email_chat, layout: layout))
    end.join
    %(<div class="max-w-lg divide-y divide-border px-2">#{rows}</div>).html_safe
  end
end
