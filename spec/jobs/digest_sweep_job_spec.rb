# frozen_string_literal: true

require "rails_helper"

RSpec.describe DigestSweepJob, type: :job do
  include ActiveSupport::Testing::TimeHelpers

  let(:ws) { Workspace.create!(name: "Sweep WS") }
  let(:user) { ws.users.create!(name: "Sweep", email_address: "sweep@example.com", password: "password123") }

  before { ActiveJob::Base.queue_adapter.enqueued_jobs.clear }

  def build_digest(next_run_at:, enabled: true)
    ws.scheduled_digests.create!(
      user:        user,
      name:        "Sweep digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: next_run_at,
      enabled:     enabled,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  it "no-op when ENABLE_DIGESTS is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      digest = build_digest(next_run_at: 1.hour.ago)
      original_next_run = digest.next_run_at

      described_class.perform_now

      expect(ActiveJob::Base.queue_adapter.enqueued_jobs.map { |j| j[:job] }).not_to include(DigestRunJob)
      # next_run_at should be unchanged
      expect(digest.reload.next_run_at.to_i).to eq(original_next_run.to_i)
    end
  end

  it "enqueues DigestRunJob for due digests and advances schedule" do
    with_env("ENABLE_DIGESTS" => "1") do
      travel_to Time.zone.parse("2026-07-06 10:00:00") do
        old_run = 1.hour.ago
        digest  = build_digest(next_run_at: old_run)

        expect { described_class.perform_now }
          .to have_enqueued_job(DigestRunJob).with(digest.id, old_run.iso8601)

        # Schedule should have been advanced
        expect(digest.reload.next_run_at).to be > Time.current
      end
    end
  end

  it "skips disabled digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      build_digest(next_run_at: 1.hour.ago, enabled: false)
      expect { described_class.perform_now }.not_to have_enqueued_job(DigestRunJob)
    end
  end

  it "skips future digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      build_digest(next_run_at: 1.hour.from_now)
      expect { described_class.perform_now }.not_to have_enqueued_job(DigestRunJob)
    end
  end
end
