# frozen_string_literal: true

# Previews for Campbooks::MessageBubble — the shared message renderer. :chat is the
# email reading pane's directional bubble; :flow is the People conversation block.
class MessageBubblePreview < Lookbook::Preview
  def chat_received
    render(Campbooks::MessageBubble.new(message: sample(sent: false), sent: false, variant: :chat, expanded: true))
  end

  def chat_sent
    render(Campbooks::MessageBubble.new(message: sample(sent: true), sent: true, variant: :chat, expanded: true))
  end

  def flow_received
    render(Campbooks::MessageBubble.new(message: sample(sent: false), sent: false, variant: :flow,
      name: "Sofia", channel_chip: true, full: true, open_thread: true))
  end

  def flow_sent
    render(Campbooks::MessageBubble.new(message: sample(sent: true), sent: true, variant: :flow,
      name: "You", channel_chip: true, full: true))
  end

  private

  def sample(sent:)
    EmailMessage.new(
      from_address: sent ? "me@myco.example" : "sofia@brightloop.example",
      subject: "Q3 deck",
      body: "Attaching the kickoff deck for Q3 — could you leave comments before Friday standup?",
      received_at: Time.current,
      channel: "email"
    )
  end
end
