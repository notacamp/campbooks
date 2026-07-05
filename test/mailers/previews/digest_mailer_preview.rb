# frozen_string_literal: true

# Mailer preview for DigestMailer#issue. Builds an in-memory sample issue with
# demo data — no real user records are required; no DB writes.
class DigestMailerPreview < ActionMailer::Preview
  def issue
    user    = User.new(id: SecureRandom.uuid, name: "Alex", email_address: "alex@example.com", locale: "en")
    ws      = Workspace.new(id: SecureRandom.uuid, name: "Demo Workspace")
    digest  = ScheduledDigest.new(
      id:             SecureRandom.uuid,
      workspace:      ws,
      user:           user,
      name:           "Weekly Roundup",
      rrule:          "FREQ=WEEKLY",
      next_run_at:    1.week.from_now,
      deliver_by_email: true
    )

    sample_content = {
      "overview" => "You have a few items that need attention this week: an invoice to pay, " \
                    "a meeting coming up, and a couple of pending tasks.",
      "sections" => [
        {
          "title" => "Finance",
          "items" => [
            {
              "source_type" => "email",
              "source_id"   => SecureRandom.uuid,
              "title"       => "Invoice #1042 from Acme Corp",
              "subtitle"    => "billing@acme.example.com",
              "note"        => "Due in 3 days — EUR 1,200",
              "timestamp"   => 2.days.ago.iso8601
            },
            {
              "source_type" => "document",
              "source_id"   => SecureRandom.uuid,
              "title"       => "Receipt — Office Supplies",
              "subtitle"    => "Receipt · EUR 89",
              "timestamp"   => 4.days.ago.iso8601
            }
          ]
        },
        {
          "title" => "Upcoming",
          "items" => [
            {
              "source_type" => "calendar_event",
              "source_id"   => SecureRandom.uuid,
              "title"       => "Investor call",
              "subtitle"    => "Tomorrow at 10:00",
              "timestamp"   => 1.day.from_now.iso8601
            },
            {
              "source_type" => "reminder",
              "source_id"   => SecureRandom.uuid,
              "title"       => "Insurance renewal due",
              "subtitle"    => "In 5 days · Renewal",
              "timestamp"   => 5.days.from_now.iso8601
            }
          ]
        },
        {
          "title" => "Tasks",
          "items" => (1..10).map do |i|
            {
              "source_type" => "task",
              "source_id"   => SecureRandom.uuid,
              "title"       => "Demo task #{i} — review partnership agreement",
              "subtitle"    => i.odd? ? "Due tomorrow · High" : "Due next week",
              "timestamp"   => i.days.from_now.iso8601
            }
          end
        }
      ],
      "meta" => { "counts" => { "emails" => 1, "documents" => 1, "calendar" => 1, "reminders" => 1, "tasks" => 10 },
                  "list_mode" => false, "source_errors" => [] }
    }

    issue = DigestIssue.new(
      id:               SecureRandom.uuid,
      scheduled_digest: digest,
      workspace_id:     ws.id,
      user_id:          user.id,
      status:           DigestIssue.statuses[:generated],
      period_start:     1.week.ago,
      period_end:       Time.current,
      content:          sample_content,
      ai_used:          true
    )

    DigestMailer.issue(issue)
  end
end
