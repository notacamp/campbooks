# frozen_string_literal: true

class ComposeContextRailPreview < ViewComponent::Preview
  # Simulates a stub email message for preview purposes.
  class StubMessage
    def id = "preview-msg-1"
    def from_address = "Jamie Torres <jamie@example.com>"
    def subject = "Re: Venue contract update"
    def received_at = 2.days.ago
    def body
      <<~HTML
        <p>Hi Alex,</p>
        <p>Quick update on the venue — they confirmed the <b>deposit date</b> moved
        to the 15th and the cancellation window is now 14 days. Everything else is
        unchanged from the draft you reviewed.</p>
        <p>Let me know if you want to hop on a call before signing.</p>
        <p>Thanks,<br>Jamie</p>
      HTML
    end
    def email_account_id = nil
  end

  # Reply mode — shows original message, attachments, and Scout cards.
  def reply
    render(Campbooks::Compose::ContextRail.new(
      message: StubMessage.new,
      mode: "reply",
      upload_url: "/compose/attachments",
      form_id: "compose_desk_form_preview",
      attachment_entries: [],
      ai_available: true
    ))
  end

  # Forward mode — same as reply but labelled "Forward".
  def forward
    render(Campbooks::Compose::ContextRail.new(
      message: StubMessage.new,
      mode: "forward",
      upload_url: "/compose/attachments",
      form_id: "compose_desk_form_preview",
      attachment_entries: [],
      ai_available: true
    ))
  end

  # New message — no original email card; only attachments + Scout.
  def new_message
    render(Campbooks::Compose::ContextRail.new(
      message: nil,
      mode: "new_message",
      upload_url: "/compose/attachments",
      form_id: "compose_desk_form_preview",
      attachment_entries: [],
      ai_available: true
    ))
  end

  # No AI configured — Scout card hidden.
  def no_ai
    render(Campbooks::Compose::ContextRail.new(
      message: StubMessage.new,
      mode: "reply",
      upload_url: "/compose/attachments",
      form_id: "compose_desk_form_preview",
      attachment_entries: [],
      ai_available: false
    ))
  end

  # Pre-seeded attachments (forwarded originals or restored draft).
  def with_attachments
    render(Campbooks::Compose::ContextRail.new(
      message: StubMessage.new,
      mode: "forward",
      upload_url: "/compose/attachments",
      form_id: "compose_desk_form_preview",
      attachment_entries: [
        { "signed_id" => "abc123", "filename" => "contract-draft.pdf", "byte_size" => 234_567 },
        { "signed_id" => "def456", "filename" => "venue-photos.zip", "byte_size" => 1_234_567 }
      ],
      ai_available: true
    ))
  end
end
