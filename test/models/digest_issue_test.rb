# frozen_string_literal: true

require "test_helper"

class DigestIssueTest < ActiveSupport::TestCase
  setup do
    @ws     = Workspace.create!(name: "Issue Model WS")
    @user   = @ws.users.create!(name: "Rui", email_address: "rui-di@example.com", password: "password123")
    @digest = @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Test digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  def build_issue(attrs = {})
    @digest.issues.build({
      workspace_id: @ws.id,
      user_id:      @user.id,
      period_start: 1.week.ago,
      period_end:   Time.current
    }.merge(attrs))
  end

  test "status enum prefix works" do
    issue = build_issue
    issue.status = :generated
    assert issue.status_generated?
    assert_not issue.status_pending?
  end

  test "overview returns empty string when content is empty" do
    issue = build_issue
    assert_equal "", issue.overview
  end

  test "sections returns empty array when content has no sections" do
    issue = build_issue
    assert_equal [], issue.sections
  end

  test "item_count sums items across sections" do
    issue = build_issue(content: {
      "sections" => [
        { "items" => [ {}, {} ] },
        { "items" => [ {} ] }
      ]
    })
    assert_equal 3, issue.item_count
  end

  test "list_mode? returns true when meta.list_mode is true" do
    issue = build_issue(content: { "meta" => { "list_mode" => true } })
    assert issue.list_mode?
  end

  test "list_mode? returns false when meta.list_mode is false" do
    issue = build_issue(content: { "meta" => { "list_mode" => false } })
    assert_not issue.list_mode?
  end
end
