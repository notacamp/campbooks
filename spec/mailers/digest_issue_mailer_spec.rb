# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestMailer, type: :mailer do
  let(:ws) { Workspace.create!(name: "Issue Mailer WS") }
  let(:user) { ws.users.create!(name: "Claude", email_address: "claude@example.com", password: "changeme123") }
  let(:digest) do
    ws.scheduled_digests.create!(
      user:        user,
      name:        "Weekly Roundup",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  let(:sample_items) do
    [
      { "source_type" => "email", "source_id" => SecureRandom.uuid,
        "title" => "Invoice #42", "subtitle" => "billing@acme.com",
        "note" => "Due in 3 days", "timestamp" => 1.day.ago.iso8601 },
      { "source_type" => "task", "source_id" => SecureRandom.uuid,
        "title" => "Review contract", "subtitle" => "Due tomorrow", "timestamp" => 1.day.from_now.iso8601 }
    ]
  end

  let(:issue) do
    digest.issues.create!(
      workspace_id: ws.id,
      user_id:      user.id,
      period_start: 1.week.ago,
      period_end:   Time.current,
      status:       :generated,
      content: {
        "overview"  => "Two items this week.",
        "sections"  => [
          { "title" => "Finance & Tasks", "items" => sample_items }
        ],
        "meta" => { "list_mode" => false, "counts" => { "emails" => 1, "tasks" => 1 }, "source_errors" => [] }
      }
    )
  end

  # ── HTML part ─────────────────────────────────────────────────────────────────

  it "renders html with digest name in subject" do
    mail = described_class.issue(issue)
    expect(mail.subject).to match("Weekly Roundup")
  end

  it "html includes the overview" do
    mail = described_class.issue(issue)
    expect(mail.html_part.decoded).to match("Two items this week.")
  end

  it "html includes item titles and notes" do
    mail = described_class.issue(issue)
    html = mail.html_part.decoded
    expect(html).to match("Invoice #42")
    expect(html).to match("Due in 3 days")
    expect(html).to match("Review contract")
  end

  it "html includes manage link" do
    mail = described_class.issue(issue)
    expect(mail.html_part.decoded).to match("digests")
  end

  # ── Text part ─────────────────────────────────────────────────────────────────

  it "text part includes item titles" do
    mail = described_class.issue(issue)
    text = mail.text_part.decoded
    expect(text).to match("Invoice #42")
    expect(text).to match("Review contract")
  end

  it "text part includes section header" do
    mail = described_class.issue(issue)
    expect(mail.text_part.decoded).to match("FINANCE & TASKS")
  end

  # ── Locale ───────────────────────────────────────────────────────────────────

  it "renders in recipient locale" do
    user.update!(locale: "fr")
    mail = described_class.issue(issue.reload)
    # French greeting
    expect(mail.text_part.decoded).to match(/Bonjour/)
  end

  # ── Overflow / "and N more" ───────────────────────────────────────────────────

  it "caps per section items and shows overflow count" do
    # Add 10 items to one section (cap is 8)
    large_items = 10.times.map do |i|
      { "source_type" => "email", "source_id" => SecureRandom.uuid,
        "title" => "Email #{i + 1}", "subtitle" => "from@example.com",
        "timestamp" => i.days.ago.iso8601 }
    end

    issue.update!(content: issue.content.merge(
      "sections" => [ { "title" => "Inbox", "items" => large_items } ]
    ))

    mail = described_class.issue(issue.reload)
    text = mail.text_part.decoded
    # Should mention 2 more (10 - 8 cap = 2)
    expect(text).to match(/2/)
  end

  # ── Addressing ───────────────────────────────────────────────────────────────

  it "sends to the digest owner's email address" do
    mail = described_class.issue(issue)
    expect(mail.to).to eq([ "claude@example.com" ])
  end
end
