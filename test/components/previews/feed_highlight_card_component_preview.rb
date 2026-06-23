# frozen_string_literal: true

# Preview for the home feed's Rewind highlight card (Campbooks::Feed::HighlightCard):
# a past standout the scroll-back resurfaces, leading with the REASON it was kept.
# One method per reason so every kicker treatment (Ember, gold star, neutral) is
# shown. In-memory records (id set so email_message_path resolves), mirroring
# FeedTimelineCardPreview.
class FeedHighlightCardComponentPreview < ViewComponent::Preview
  # Starred sender — gold accent kicker.
  def starred
    card(reason: :starred, subject: "Re: Spring charter — final balance",
         from: '"Maria Santos" <maria@example.com>')
  end

  # Important — Ember (the priority signature) kicker.
  def important
    card(reason: :important, subject: "Allergy update for Sami before week 3",
         from: '"The Okafor family" <okafor@example.com>')
  end

  # High priority — Ember kicker.
  def high_priority
    card(reason: :high_priority, subject: "Action needed: lease renewal deadline Friday",
         from: '"Jamie Vela" <jamie@studio.example>')
  end

  # Attachment — neutral kicker (invoice / contract / doc).
  def attachment
    card(reason: :attachment, subject: "Invoice #2025-114", has_attachment: true,
         from: '"Maple Lodge" <billing@maplelodge.example>')
  end

  # Busy thread — neutral kicker (a long, real conversation).
  def busy_thread
    card(reason: :busy_thread, subject: "Partnership planning — next steps",
         from: '"Greenline Camps" <hi@greenline.example>')
  end

  private

  def card(reason:, subject:, from:, has_attachment: false)
    render Campbooks::Feed::HighlightCard.new(reason: reason, email: EmailMessage.new(
      id: 9001,
      from_address: from,
      subject: subject,
      ai_summary: "Scout kept this one: it's from someone or about something that mattered, " \
                  "worth a second look as you scroll back through the year.",
      has_attachment: has_attachment,
      received_at: Time.utc(2024, 9, 12, 9, 41)
    ))
  end
end
