# frozen_string_literal: true

require "test_helper"

class Campbooks::Feed::DigestCardTest < ActiveSupport::TestCase
  test "renders digest name, overview, item titles and issue link" do
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

    html = ApplicationController.render(Campbooks::Feed::DigestCard.new(item: item, subject: issue), layout: false)

    assert_includes html, "Week ahead"
    assert_includes html, "Two meetings and one overdue task."
    assert_includes html, "Planning workshop"
    assert_includes html, "/digests/#{digest.id}/issues/#{issue.id}"
  end

  test "falls back to item count when overview is blank" do
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

    html = ApplicationController.render(Campbooks::Feed::DigestCard.new(item: item, subject: issue), layout: false)

    assert_includes html, "Invoice tracker"
    assert_includes html, "4 items"
  end
end
