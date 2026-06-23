# frozen_string_literal: true

class BoardColumnComponentPreview < ViewComponent::Preview
  # The Inbox column with a few draggable cards.
  def inbox
    render(Campbooks::BoardColumn.new(column: { key: :inbox, threads: sample_threads(3), has_more: false, draggable: true }))
  end

  # The read-only Awaiting column (lock hint, no drops in).
  def awaiting
    render(Campbooks::BoardColumn.new(column: { key: :awaiting, threads: sample_threads(2), has_more: false, draggable: false }))
  end

  # A column that hit the per-column cap shows a "+ more" footer.
  def overflow
    render(Campbooks::BoardColumn.new(column: { key: :snoozed, threads: sample_threads(4), has_more: true, draggable: true }))
  end

  # An empty column.
  def empty
    render(Campbooks::BoardColumn.new(column: { key: :done, threads: [], has_more: false, draggable: true }))
  end

  private

  def sample_threads(count)
    subjects = [
      "Invoice #2025-114 needs your sign-off",
      "Re: Charter balance due",
      "Allergy update before week 3",
      "Lunch on Thursday?",
      "Newsletter: July highlights"
    ]
    senders = %w[emma@maplelodge.com ops@blueox.com okafor@family.com sam@team.com news@digest.com]

    Array.new(count) do |i|
      account = EmailAccount.new(id: i + 1, color: "#6366f1", email_address: "me@example.com")
      thread = EmailThread.new
      thread.email_account = account
      message = EmailMessage.new(
        id: i + 1,
        from_address: senders[i % senders.size],
        subject: subjects[i % subjects.size],
        received_at: (i + 1).hours.ago,
        provider_folder_id: "INBOX"
      )
      message.email_account = account
      # Mark the collection loaded so latest_message stays in-memory.
      thread.association(:email_messages).target = [ message ]
      thread.association(:email_messages).loaded!
      thread
    end
  end
end
