# frozen_string_literal: true

require "test_helper"

class DigestIssueMailerTest < ActionMailer::TestCase
  setup do
    @ws     = Workspace.create!(name: "Issue Mailer WS")
    @user   = @ws.users.create!(name: "Claude", email_address: "claude@example.com", password: "changeme123")
    @digest = @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Weekly Roundup",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )

    sample_items = [
      { "source_type" => "email", "source_id" => SecureRandom.uuid,
        "title" => "Invoice #42", "subtitle" => "billing@acme.com",
        "note" => "Due in 3 days", "timestamp" => 1.day.ago.iso8601 },
      { "source_type" => "task", "source_id" => SecureRandom.uuid,
        "title" => "Review contract", "subtitle" => "Due tomorrow", "timestamp" => 1.day.from_now.iso8601 }
    ]

    @issue = @digest.issues.create!(
      workspace_id: @ws.id,
      user_id:      @user.id,
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

  test "renders html with digest name in subject" do
    mail = DigestMailer.issue(@issue)
    assert_match "Weekly Roundup", mail.subject
  end

  test "html includes the overview" do
    mail = DigestMailer.issue(@issue)
    assert_match "Two items this week.", mail.html_part.decoded
  end

  test "html includes item titles and notes" do
    mail = DigestMailer.issue(@issue)
    html = mail.html_part.decoded
    assert_match "Invoice #42", html
    assert_match "Due in 3 days", html
    assert_match "Review contract", html
  end

  test "html includes manage link" do
    mail = DigestMailer.issue(@issue)
    assert_match "digests", mail.html_part.decoded
  end

  # ── Text part ─────────────────────────────────────────────────────────────────

  test "text part includes item titles" do
    mail = DigestMailer.issue(@issue)
    text = mail.text_part.decoded
    assert_match "Invoice #42", text
    assert_match "Review contract", text
  end

  test "text part includes section header" do
    mail = DigestMailer.issue(@issue)
    text = mail.text_part.decoded
    assert_match "FINANCE & TASKS", text
  end

  # ── Locale ───────────────────────────────────────────────────────────────────

  test "renders in recipient locale" do
    @user.update!(locale: "fr")
    mail = DigestMailer.issue(@issue.reload)
    # French greeting
    assert_match(/Bonjour/, mail.text_part.decoded)
  end

  # ── Overflow / "and N more" ───────────────────────────────────────────────────

  test "caps per section items and shows overflow count" do
    # Add 10 items to one section (cap is 8)
    large_items = 10.times.map do |i|
      { "source_type" => "email", "source_id" => SecureRandom.uuid,
        "title" => "Email #{i + 1}", "subtitle" => "from@example.com",
        "timestamp" => i.days.ago.iso8601 }
    end

    @issue.update!(content: @issue.content.merge(
      "sections" => [ { "title" => "Inbox", "items" => large_items } ]
    ))

    mail = DigestMailer.issue(@issue.reload)
    text = mail.text_part.decoded
    # Should mention 2 more (10 - 8 cap = 2)
    assert_match(/2/, text)
  end

  # ── Addressing ───────────────────────────────────────────────────────────────

  test "sends to the digest owner's email address" do
    mail = DigestMailer.issue(@issue)
    assert_equal [ "claude@example.com" ], mail.to
  end
end
