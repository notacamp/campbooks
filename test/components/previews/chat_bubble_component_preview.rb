class ChatBubbleComponentPreview < ViewComponent::Preview
  def ai_bubble
    render Campbooks::ChatBubble.new(
      author: "Scout",
      role: :ai,
      timestamp: "2 min ago"
    ) { "I've reviewed the document and found three key clauses that need attention." }
  end

  def user_bubble
    render Campbooks::ChatBubble.new(
      author: "You",
      role: :user,
      timestamp: "3 min ago"
    ) { "Can you check page 4 of the contract?" }
  end

  def typing_indicator
    render Campbooks::ChatBubble::TypingIndicator.new
  end

  def side_by_side
    html = [
      render(Campbooks::ChatBubble.new(author: "Scout", role: :ai, timestamp: "2 min ago") { "Hello! How can I help you today?" }),
      render(Campbooks::ChatBubble.new(author: "You", role: :user, timestamp: "1 min ago") { "Can you summarize the latest email thread?" }),
      render(Campbooks::ChatBubble::TypingIndicator.new)
    ].join
    "<div class=\"space-y-4 max-w-lg\">#{html}</div>".html_safe
  end
end
