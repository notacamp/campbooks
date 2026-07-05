# frozen_string_literal: true

require "test_helper"

class DigestIssueMailJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper
  include ActionMailer::TestHelper

  setup do
    @ws     = Workspace.create!(name: "Mail Job WS")
    @user   = @ws.users.create!(name: "Mailer", email_address: "mailer@example.com", password: "password123")
    @digest = @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Mail digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] },
      deliver_by_email: true
    )
    @issue = @digest.issues.create!(
      workspace_id: @ws.id,
      user_id:      @user.id,
      period_start: 1.week.ago,
      period_end:   Time.current,
      status:       :generated,
      content: {
        "overview"  => "Test overview",
        "sections"  => [],
        "meta"      => { "list_mode" => false, "counts" => {}, "source_errors" => [] }
      }
    )
  end

  test "sends email for a generated issue" do
    assert_emails 1 do
      DigestIssueMailJob.perform_now(@issue.id)
    end
    assert_not_nil @issue.reload.email_sent_at
  end

  test "does not send when issue status is not generated" do
    @issue.update!(status: :empty)
    assert_emails 0 do
      DigestIssueMailJob.perform_now(@issue.id)
    end
    assert_nil @issue.reload.email_sent_at
  end

  test "does not send when digest deliver_by_email is false" do
    @digest.update!(deliver_by_email: false)
    assert_emails 0 do
      DigestIssueMailJob.perform_now(@issue.id)
    end
  end

  test "does not send when digest is disabled" do
    @digest.update!(enabled: false)
    assert_emails 0 do
      DigestIssueMailJob.perform_now(@issue.id)
    end
  end

  test "no-op for non-existent issue" do
    assert_emails 0 do
      DigestIssueMailJob.perform_now(SecureRandom.uuid)
    end
  end

  test "stamps email_sent_at on success" do
    travel_to Time.zone.parse("2026-07-06 10:00:00") do
      DigestIssueMailJob.perform_now(@issue.id)
      assert_in_delta Time.current.to_f, @issue.reload.email_sent_at.to_f, 2
    end
  end
end
