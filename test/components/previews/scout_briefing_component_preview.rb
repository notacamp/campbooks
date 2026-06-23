# @label Scout Briefing
class ScoutBriefingComponentPreview < Lookbook::Preview
  # The proactive empty state for a fresh Scout chat.
  def default
    render Campbooks::ScoutBriefing.new(
      greeting: "Good evening, Alex",
      subtitle: "3 emails flagged high-priority. Ask me anything about your inbox, documents, or contacts — I can search, summarize, and take action.",
      stats: [
        { value: 3, label: "High priority", icon: :flag, tone: :amber, prompt: "What are my high-priority emails right now?" },
        { value: 217, label: "Unread", icon: :inbox, tone: :accent, prompt: "Summarize my unread emails" },
        { value: 4, label: "Docs to review", icon: :document, tone: :default, prompt: "Which documents still need review?" }
      ],
      suggestions: [
        "What needs my attention today?",
        "Find my recent invoices",
        "Which newsletters can I archive?",
        "Summarize this week's emails"
      ]
    )
  end

  # When the inbox is clear.
  def all_caught_up
    render Campbooks::ScoutBriefing.new(
      greeting: "Good morning, Alex",
      subtitle: "You're all caught up. Ask me anything about your emails, documents, or contacts — I can search, summarize, and run reports.",
      stats: [
        { value: 0, label: "High priority", icon: :flag, tone: :amber, prompt: "What are my high-priority emails right now?" },
        { value: 0, label: "Unread", icon: :inbox, tone: :accent, prompt: "Summarize my unread emails" }
      ],
      suggestions: [
        "Summarize this week's emails",
        "Find my recent invoices"
      ]
    )
  end
end
