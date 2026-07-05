# frozen_string_literal: true

# Previews for the home-feed digest card. In-memory FeedItem + DigestIssue.
class DigestFeedCardComponentPreview < ViewComponent::Preview
  # A fresh AI-curated issue: name, overview, first item titles, CTA.
  def with_overview
    render Campbooks::Feed::DigestCard.new(item: feed_item, subject: issue)
  end

  # A list-mode issue (no overview) falls back to the item-count line.
  def without_overview
    render Campbooks::Feed::DigestCard.new(
      item: feed_item(data: { "digest_name" => "Invoice tracker", "overview" => "", "item_count" => 4 }),
      subject: issue(overview: "")
    )
  end

  private

  def feed_item(data: nil)
    data ||= {
      "digest_name" => "Week ahead",
      "overview" => "Light calendar until Thursday, then three meetings back-to-back. Two tasks land Monday.",
      "item_count" => 6
    }
    FeedItem.new(
      id: "55555555-5555-4555-8555-555555555555",
      kind: "digest_issue",
      data: data,
      sort_at: Time.current,
      created_at: Time.current
    )
  end

  def issue(overview: "Light calendar until Thursday, then three meetings back-to-back. Two tasks land Monday.")
    DigestIssue.new(
      id: "66666666-6666-4666-8666-666666666666",
      scheduled_digest_id: "77777777-7777-4777-8777-777777777777",
      status: :generated,
      period_start: 1.week.ago,
      period_end: Time.current,
      created_at: 2.hours.ago,
      content: {
        "overview" => overview,
        "sections" => [
          { "key" => "calendar", "items" => [
            { "source_type" => "calendar_event", "source_id" => "x", "title" => "Q3 planning workshop" },
            { "source_type" => "calendar_event", "source_id" => "y", "title" => "1:1 with Dana" }
          ] },
          { "key" => "tasks", "items" => [
            { "source_type" => "task", "source_id" => "z", "title" => "Prepare workshop agenda" }
          ] }
        ]
      }
    )
  end
end
