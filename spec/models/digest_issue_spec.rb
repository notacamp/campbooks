# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestIssue do
  before do
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

  it "status enum prefix works" do
    issue = build_issue
    issue.status = :generated
    expect(issue).to be_status_generated
    expect(issue).not_to be_status_pending
  end

  it "overview returns empty string when content is empty" do
    issue = build_issue
    expect(issue.overview).to eq("")
  end

  it "sections returns empty array when content has no sections" do
    issue = build_issue
    expect(issue.sections).to eq([])
  end

  it "item_count sums items across sections" do
    issue = build_issue(content: {
      "sections" => [
        { "items" => [ {}, {} ] },
        { "items" => [ {} ] }
      ]
    })
    expect(issue.item_count).to eq(3)
  end

  it "list_mode? returns true when meta.list_mode is true" do
    issue = build_issue(content: { "meta" => { "list_mode" => true } })
    expect(issue).to be_list_mode
  end

  it "list_mode? returns false when meta.list_mode is false" do
    issue = build_issue(content: { "meta" => { "list_mode" => false } })
    expect(issue).not_to be_list_mode
  end
end
