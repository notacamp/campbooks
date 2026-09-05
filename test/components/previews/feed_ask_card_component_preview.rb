# frozen_string_literal: true

# Previews for the home-feed ask card, one per framing (Feed::Sources::Task stamps
# the framing into the item's data). In-memory records stand in; the Hold slot is
# only computed with a real user in context, so previews show the date/complete
# controls rather than a live slot.
class FeedAskCardComponentPreview < ViewComponent::Preview
  # Scout found a fresh ask, no date — Not now / Set a date / Hold.
  def suggested_undated
    render_card(task(status: :suggested, ai_suggested: true), framing: "suggested")
  end

  # A fresh ask with a date Scout read off the mail.
  def suggested_dated
    render_card(task(status: :suggested, ai_suggested: true, due_at: 3.days.from_now), framing: "suggested")
  end

  # An accepted ask still without a date — it needs a "when".
  def open_undated
    render_card(task(status: :todo), framing: "undated")
  end

  # An accepted ask due today — the primary is Done.
  def due_today
    render_card(task(status: :todo, due_at: Time.current.end_of_day), framing: "due")
  end

  # An overdue ask — the kicker turns red.
  def overdue
    render_card(task(status: :todo, due_at: 2.days.ago), framing: "due")
  end

  private

  def render_card(subject, framing:)
    item = FeedItem.new(id: SecureRandom.uuid, kind: "task", data: { "framing" => framing })
    render Campbooks::Feed::AskCard.new(item: item, subject: subject)
  end

  def task(**attrs)
    Task.new({ id: SecureRandom.uuid, title: "Send the signed contract back to Acme",
               justification: "Rita asked you to countersign and send it back." }.merge(attrs))
  end
end
