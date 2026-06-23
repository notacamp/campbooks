# frozen_string_literal: true

# Previews for the home-feed cards, each rendered through the Campbooks::Feed::Card
# dispatcher exactly as the timeline renders them. In-memory records (ids set so the
# action route helpers resolve) stand in for real feed items + subjects.
class FeedTimelineCardPreview < ViewComponent::Preview
  # Imminent meeting nudge with a Join action.
  def calendar_event
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 8, kind: "calendar_event", attention: true),
      subject: calendar_event_subject
    )
  end

  # Bordered hero: a Scout-flagged email with its read + inline actions.
  def email_action
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 1, kind: "email_action"),
      subject: email(
        subject: "Invoice #2025-114 needs your sign-off",
        ai_summary: "Maple Lodge sent invoice #2025-114 for the July booking, €4,200, net 7 days.",
        ai_action_prompt: "This matches your approved March quote. I drafted an approval reply.",
        ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => "invoices" } } ],
        category: "important",
        has_attachment: true
      )
    )
  end

  # Same card, promoted to the attention cluster (Ember priority dot).
  def email_action_priority
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 2, kind: "email_action", attention: true),
      subject: email(
        subject: "Allergy update for Sami before week 3",
        ai_summary: "A parent flagged a new tree-nut allergy since registration.",
        ai_action_prompt: "Health-critical. I drafted a reassuring reply and can notify the kitchen.",
        ai_priority: "high"
      )
    )
  end

  # The promoted starred-sender card: its own surface + star badge + Open/Unstar.
  def starred_email
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 7, kind: "starred_email", attention: true, data: { "starred_sender" => true }),
      subject: email(
        subject: "Contract draft ready for your signature",
        from_address: "Jamie Vela <jamie@studio.example>",
        ai_action_prompt: "Jamie sent the final contract — she needs your signature by Friday.",
        category: "important"
      )
    )
  end

  # Borderless quiet nudge: aged, still-unanswered mail.
  def reply_reminder
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 3, kind: "reply_reminder", data: { "reason" => "no_reply", "age_days" => 9 }),
      subject: email(subject: "Waterfront permit note", from_address: "Dana Whitfield <dana@example.com>")
    )
  end

  # Borderless nudge variant: a thread back from snooze.
  def reply_reminder_snoozed
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 4, kind: "reply_reminder", data: { "reason" => "snooze_due" }),
      subject: email(subject: "Partnership follow-up", from_address: "Greenline Camps <hi@greenline.example>")
    )
  end

  # Borderless nudge: a conversation YOU replied to and are still waiting on — the
  # AI judged a follow-up worth sending. Primary "Draft follow-up" + Dismiss.
  def follow_up
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 11, kind: "follow_up", attention: true,
                      data: { "reason" => "You asked them to confirm the final balance.", "age_days" => 4 }),
      subject: email(subject: "Re: Spring charter — final balance", from_address: "Maria Santos <maria@example.com>")
    )
  end

  # Compact, borderless one-line filing suggestion.
  def tag_suggestion
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 5, kind: "tag_suggestion", data: { "tag_name" => "receipts" }),
      subject: email(subject: "Your receipt from BlueOx Buses")
    )
  end

  # A consecutive run of filing suggestions, stacked as one tight, divider-free
  # queue (how the timeline renders any streak of tag_suggestion items).
  def tag_queue
    render Campbooks::Feed::TagQueueCard.new(items: [
      { item: feed_item(id: 51, kind: "tag_suggestion", data: { "tag_name" => "receipts" }),
        subject: email(subject: "Your receipt from BlueOx Buses") },
      { item: feed_item(id: 52, kind: "tag_suggestion", data: { "tag_name" => "security" }),
        subject: email(subject: "Your password has been changed") },
      { item: feed_item(id: 53, kind: "tag_suggestion", data: { "tag_name" => "newsletters" }),
        subject: email(subject: "This week at Maple Lodge — issue #12") }
    ])
  end

  # The same queue with a single suggestion — still the compact row, not a card.
  def tag_queue_single
    render Campbooks::Feed::TagQueueCard.new(items: [
      { item: feed_item(id: 54, kind: "tag_suggestion", data: { "tag_name" => "receipts" }),
        subject: email(subject: "Your receipt from BlueOx Buses") }
    ])
  end

  # AI-detected reminder awaiting confirmation: type, due date, amount, and a
  # one-tap "Add to calendar" / "Dismiss". Borderless nudge styling.
  def reminder
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 9, kind: "reminder", attention: true),
      subject: reminder_subject
    )
  end

  # Overdue reminder — the due chip turns red.
  def reminder_overdue
    render Campbooks::Feed::Card.new(
      item: feed_item(id: 10, kind: "reminder", attention: true),
      subject: reminder_subject(title: "Renew liability insurance", reminder_type: "renewal", due_at: 2.days.ago, amount_cents: nil)
    )
  end

  # @label Focused — keyboard shortcut chips
  #
  # The card the reader is on (the scroll-highlight feed-keyboard acts on). On
  # fine-pointer devices its shortcut chips surface — → primary, ← escape, a
  # letter for the rest — plus a ↑↓ nav hint; touch shows a swipe hint instead.
  # The sidecar template forces the data-focused the scroll controller sets live.
  def focused_with_chips
    render_with_template(locals: {
      item: feed_item(id: 1, kind: "email_action"),
      subject: email(
        subject: "Invoice #2025-114 needs your sign-off",
        ai_action_prompt: "This matches your approved March quote. I drafted an approval reply.",
        ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => "invoices" } } ],
        category: "important"
      )
    })
  end

  private

  def reminder_subject(title: "Pay EDP invoice — January", reminder_type: "payment_due",
                       due_at: 3.days.from_now, amount_cents: 18_450)
    Reminder.new(
      id: 9300, title: title, reminder_type: reminder_type, due_at: due_at, all_day: true,
      status: "pending", amount_cents: amount_cents, currency: "EUR", source_type: "EmailMessage",
      justification: "The invoice states payment is due within 15 days of issue."
    )
  end

  def feed_item(id:, kind:, attention: false, data: {})
    FeedItem.new(id: id, kind: kind, attention: attention, data: data)
  end

  def email(subject:, from_address: "Emma · Maple Lodge <emma@maplelodge.example>",
            ai_summary: nil, ai_action_prompt: nil, ai_suggested_actions: [],
            ai_priority: "medium", category: nil, has_attachment: false)
    EmailMessage.new(
      id: 9001,
      from_address: from_address,
      subject: subject,
      ai_summary: ai_summary,
      ai_action_prompt: ai_action_prompt,
      ai_suggested_actions: ai_suggested_actions,
      ai_priority: ai_priority,
      category: category,
      has_attachment: has_attachment,
      received_at: Time.current - rand(1..6).hours
    )
  end

  def calendar_event_subject
    cal = Calendar.new(id: 1, name: "Personal", color: "#0584da", calendar_account: CalendarAccount.new(id: 1, color: "#0584da"))
    CalendarEvent.new(
      id: 9100, title: "Standup with the team", location: "Google Meet",
      conference_url: "https://meet.google.com/abc",
      start_at: Time.current + 1200, end_at: Time.current + 3000, status: :confirmed, calendar: cal
    )
  end
end
