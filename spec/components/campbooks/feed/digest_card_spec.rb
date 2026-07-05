# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::DigestCard, type: :component do
  it "renders digest name, overview, item titles and issue link" do
    digest = ScheduledDigest.new(id: SecureRandom.uuid, name: "Week ahead", rrule: "FREQ=WEEKLY")
    issue = DigestIssue.new(
      id: SecureRandom.uuid,
      scheduled_digest: digest,
      status: :generated,
      created_at: 1.hour.ago,
      content: {
        "overview" => "Two meetings and one overdue task.",
        "sections" => [
          { "key" => "calendar", "items" => [ { "source_type" => "calendar_event", "source_id" => "a", "title" => "Planning workshop" } ] }
        ]
      }
    )
    item = FeedItem.new(
      id: SecureRandom.uuid,
      kind: "digest_issue",
      data: { "digest_name" => "Week ahead", "overview" => "Two meetings and one overdue task.", "item_count" => 3 },
      created_at: 1.hour.ago
    )

    html = ApplicationController.render(described_class.new(item: item, subject: issue), layout: false)

    expect(html).to include("Week ahead")
    expect(html).to include("Two meetings and one overdue task.")
    expect(html).to include("Planning workshop")
    expect(html).to include("/digests/#{digest.id}/issues/#{issue.id}")
  end

  it "falls back to item count when overview is blank" do
    issue = DigestIssue.new(
      id: SecureRandom.uuid,
      scheduled_digest_id: SecureRandom.uuid,
      status: :generated,
      created_at: 1.hour.ago,
      content: { "overview" => "", "sections" => [] }
    )
    item = FeedItem.new(
      id: SecureRandom.uuid,
      kind: "digest_issue",
      data: { "digest_name" => "Invoice tracker", "overview" => "", "item_count" => 4 },
      created_at: 1.hour.ago
    )

    html = ApplicationController.render(described_class.new(item: item, subject: issue), layout: false)

    expect(html).to include("Invoice tracker")
    expect(html).to include("4 items")
  end
end
