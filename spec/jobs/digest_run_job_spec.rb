# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestRunJob, type: :job do
  let(:ws) { Workspace.create!(name: "Run WS") }
  let(:user) { ws.users.create!(name: "Runner", email_address: "runner@example.com", password: "password123") }
  let(:digest) do
    ws.scheduled_digests.create!(
      user:             user,
      name:             "Run digest",
      rrule:            "FREQ=WEEKLY",
      next_run_at:      1.week.from_now,
      config:           { "sources" => [ { "type" => "emails", "query" => "" } ] },
      ai_enabled:       false,
      deliver_by_email: false,
      show_in_feed:     false
    )
  end

  it "no-op when feature flag is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      expect { described_class.perform_now(digest.id, Time.current.iso8601) }
        .not_to change(DigestIssue, :count)
    end
  end

  it "no-op when digest does not exist" do
    with_env("ENABLE_DIGESTS" => "1") do
      expect { described_class.perform_now(SecureRandom.uuid, Time.current.iso8601) }
        .not_to change(DigestIssue, :count)
    end
  end

  it "no-op when digest is disabled (non-manual)" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest.update!(enabled: false)
      expect { described_class.perform_now(digest.id, Time.current.iso8601) }
        .not_to change(DigestIssue, :count)
    end
  end

  it "manual run proceeds even when digest is disabled" do
    with_env("ENABLE_DIGESTS" => "1") do
      ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      digest.update!(enabled: false)
      expect { described_class.perform_now(digest.id, Time.current.iso8601, manual: true) }
        .to change(DigestIssue, :count).by(1)
    end
  end

  it "clears Current.workspace and Current.acting_user after run" do
    with_env("ENABLE_DIGESTS" => "1") do
      ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      described_class.perform_now(digest.id, Time.current.iso8601)
      expect(Current.workspace).to be_nil,   "Current.workspace must be cleared after job"
      expect(Current.acting_user).to be_nil, "Current.acting_user must be cleared after job"
    end
  end

  it "gates on workspace entitlement" do
    with_env("ENABLE_DIGESTS" => "1") do
      ws.update!(entitlement_overrides: { "digests" => { "allowed" => false } })
      expect { described_class.perform_now(digest.id, Time.current.iso8601) }
        .not_to change(DigestIssue, :count)
    end
  end

  it "generates an issue when all gates pass" do
    with_env("ENABLE_DIGESTS" => "1") do
      ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      expect { described_class.perform_now(digest.id, Time.current.iso8601) }
        .to change(DigestIssue, :count).by(1)
    end
  end
end
