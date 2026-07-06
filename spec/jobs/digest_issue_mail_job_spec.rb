# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestIssueMailJob, type: :job do
  let(:ws) { Workspace.create!(name: "Mail Job WS") }
  let(:user) { ws.users.create!(name: "Mailer", email_address: "mailer@example.com", password: "password123") }
  let(:digest) do
    ws.scheduled_digests.create!(
      user:             user,
      name:             "Mail digest",
      rrule:            "FREQ=WEEKLY",
      next_run_at:      1.week.from_now,
      config:           { "sources" => [ { "type" => "emails", "query" => "" } ] },
      deliver_by_email: true
    )
  end
  let(:issue) do
    digest.issues.create!(
      workspace_id: ws.id,
      user_id:      user.id,
      period_start: 1.week.ago,
      period_end:   Time.current,
      status:       :generated,
      content: {
        "overview" => "Test overview",
        "sections" => [],
        "meta"     => { "list_mode" => false, "counts" => {}, "source_errors" => [] }
      }
    )
  end

  it "sends email for a generated issue" do
    expect { described_class.perform_now(issue.id) }
      .to change { ActionMailer::Base.deliveries.size }.by(1)
    expect(issue.reload.email_sent_at).not_to be_nil
  end

  it "does not send when issue status is not generated" do
    issue.update!(status: :empty)
    expect { described_class.perform_now(issue.id) }
      .not_to change { ActionMailer::Base.deliveries.size }
    expect(issue.reload.email_sent_at).to be_nil
  end

  it "does not send when digest deliver_by_email is false" do
    digest.update!(deliver_by_email: false)
    expect { described_class.perform_now(issue.id) }
      .not_to change { ActionMailer::Base.deliveries.size }
  end

  it "does not send when digest is disabled" do
    digest.update!(enabled: false)
    expect { described_class.perform_now(issue.id) }
      .not_to change { ActionMailer::Base.deliveries.size }
  end

  it "no-op for non-existent issue" do
    expect { described_class.perform_now(SecureRandom.uuid) }
      .not_to change { ActionMailer::Base.deliveries.size }
  end

  it "stamps email_sent_at on success" do
    before = Time.current
    described_class.perform_now(issue.id)
    expect(issue.reload.email_sent_at).to be_present
    expect(issue.reload.email_sent_at.to_f).to be_within(5).of(before.to_f)
  end
end
