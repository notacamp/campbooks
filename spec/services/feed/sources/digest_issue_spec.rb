# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::DigestIssue do
  let(:ws) { Workspace.create!(name: "Feed Digest WS") }
  let(:user) do
    ws.users.create!(
      name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end
  let(:digest) do
    ws.scheduled_digests.create!(
      user:         user,
      name:         "Feed digest",
      rrule:        "FREQ=WEEKLY",
      next_run_at:  1.week.from_now,
      config:       { "sources" => [ { "type" => "emails", "query" => "" } ] },
      show_in_feed: true
    )
  end
  let(:source) { described_class.new(user) }

  before { digest } # trigger creation

  def generated_issue(attrs = {})
    digest.issues.create!({
      workspace_id: ws.id,
      user_id:      user.id,
      period_start: 1.week.ago,
      period_end:   Time.current,
      status:       :generated,
      content:      { "overview" => "All good", "sections" => [], "meta" => {} }
    }.merge(attrs))
  end

  def with_digests_flag(&block)
    with_env("ENABLE_DIGESTS" => "1", &block)
  end

  it "returns candidates for generated issues within the window" do
    issue = generated_issue
    candidates = with_digests_flag { source.candidates }

    expect(candidates).not_to be_empty
    subjects = candidates.map { |c| c[:subject].id }
    expect(subjects).to include(issue.id)
  end

  it "excludes empty or failed issues" do
    generated_issue(status: :empty)
    generated_issue(status: :failed)

    candidates = with_digests_flag { source.candidates }
    expect(candidates).to be_empty
  end

  it "excludes issues older than 3 days" do
    old_issue = generated_issue
    old_issue.update_column(:created_at, 4.days.ago)

    candidates = with_digests_flag { source.candidates }
    subjects = candidates.map { |c| c[:subject].id }
    expect(subjects).not_to include(old_issue.id)
  end

  it "excludes issues when show_in_feed is false" do
    digest.update!(show_in_feed: false)
    generated_issue

    candidates = with_digests_flag { source.candidates }
    expect(candidates).to be_empty
  end

  it "excludes issues when digest is disabled" do
    digest.update!(enabled: false)
    generated_issue

    candidates = with_digests_flag { source.candidates }
    expect(candidates).to be_empty
  end

  it "returns empty array when feature flag is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      generated_issue
      expect(source.candidates).to be_empty
    end
  end

  it "candidate shape has expected keys" do
    issue = generated_issue
    candidates = with_digests_flag { source.candidates }
    c = candidates.find { |x| x[:subject].id == issue.id }

    expect(c).not_to be_nil
    expect(c[:dedupe_key]).to eq("digest_issue:#{issue.id}")
    expect(c[:score]).to eq(60)
    expect(c[:attention]).to eq(false)
    expect(c[:data]["digest_name"]).to eq(digest.name)
  end

  it "still_valid? returns true for generated issue" do
    issue = generated_issue
    expect(source.still_valid?(nil, issue)).to be_truthy
  end

  it "still_valid? returns false for nil subject" do
    expect(source.still_valid?(nil, nil)).to be_falsey
  end

  it "still_valid? returns false for non-generated issue" do
    issue = generated_issue
    issue.update!(status: :empty)
    expect(source.still_valid?(nil, issue)).to be_falsey
  end
end
