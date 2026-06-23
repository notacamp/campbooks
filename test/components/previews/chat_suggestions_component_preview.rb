# @label Chat Suggestions
class ChatSuggestionsComponentPreview < Lookbook::Preview
  # Follow-up prompt chips shown under Scout's latest reply.
  def follow_ups
    render Campbooks::ChatSuggestions.new(
      prompts: [
        "Draft a reply to Jamie",
        "Archive these newsletters",
        "Show me April's invoices"
      ],
      dismissable: true
    )
  end

  # Starter prompts, centered with a heading (as used in the briefing).
  def with_heading
    render Campbooks::ChatSuggestions.new(
      prompts: [
        "What needs my attention today?",
        "Find my recent invoices",
        "Which newsletters can I archive?"
      ],
      heading: "Try asking",
      align: :center
    )
  end
end
