# frozen_string_literal: true

# Previews for Campbooks::MessageBubble — the shared message body used by the email
# reading pane (:chat).
class MessageBubbleComponentPreview < ViewComponent::Preview
  def chat_received
    render Campbooks::MessageBubble.new(message: received, sent: false, variant: :chat)
  end

  def chat_sent
    render Campbooks::MessageBubble.new(message: sent, sent: true, variant: :chat)
  end

  private

  def received
    EmailMessage.new(from_address: "sofia@brightloop.example", subject: "Q3 kickoff",
                     body: "<p>Hey! Could you leave comments on the Q3 deck before Friday standup? Mostly slides 4 to 9.</p>",
                     received_at: Time.current, channel: "email")
  end

  def sent
    EmailMessage.new(from_address: "you@example.com", subject: "Re: Q3 kickoff",
                     body: "<p>Agreed on the scope. Let's keep the second experiment to the EU cohort for now.</p>",
                     received_at: 1.day.ago, channel: "email")
  end
end
