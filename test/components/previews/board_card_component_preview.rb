# frozen_string_literal: true

class BoardCardComponentPreview < ViewComponent::Preview
  # A standard draggable card (Inbox column).
  def default
    render(Campbooks::BoardCard.new(thread: sample_thread, column_key: :inbox))
  end

  # A card in the Snoozed column shows when it will resurface.
  def snoozed
    render(Campbooks::BoardCard.new(thread: sample_thread(snoozed_until: 1.day.from_now), column_key: :snoozed))
  end

  # A card in the read-only Awaiting column (not draggable into, only out of).
  def awaiting
    render(Campbooks::BoardCard.new(thread: sample_thread, column_key: :awaiting, draggable: false))
  end

  private

  # An unsaved thread + message, just enough for the card to render.
  def sample_thread(snoozed_until: nil)
    account = EmailAccount.new(id: 1, color: "#6366f1", email_address: "me@example.com")
    thread = EmailThread.new(snoozed_until: snoozed_until)
    thread.email_account = account
    message = EmailMessage.new(
      id: 1,
      from_address: "emma@maplelodge.com",
      subject: "Invoice #2025-114 needs your sign-off",
      received_at: 2.hours.ago,
      provider_folder_id: "INBOX"
    )
    message.email_account = account
    # Mark the collection loaded so latest_message uses these in-memory records
    # instead of querying the dev database.
    thread.association(:email_messages).target = [ message ]
    thread.association(:email_messages).loaded!
    thread
  end
end
