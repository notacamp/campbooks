# frozen_string_literal: true

require "test_helper"

class DigestSweepJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    @ws   = Workspace.create!(name: "Sweep WS")
    @user = @ws.users.create!(name: "Sweep", email_address: "sweep@example.com", password: "password123")
  end

  def build_digest(next_run_at:, enabled: true)
    @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Sweep digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: next_run_at,
      enabled:     enabled,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  test "no-op when ENABLE_DIGESTS is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      digest = build_digest(next_run_at: 1.hour.ago)
      DigestSweepJob.perform_now
      assert_no_enqueued_jobs(only: DigestRunJob)
      # next_run_at should be unchanged
      assert_equal digest.next_run_at, digest.reload.next_run_at
    end
  end

  test "enqueues DigestRunJob for due digests and advances schedule" do
    with_env("ENABLE_DIGESTS" => "1") do
      travel_to Time.zone.parse("2026-07-06 10:00:00") do
        old_run = 1.hour.ago
        digest  = build_digest(next_run_at: old_run)

        DigestSweepJob.perform_now

        run_jobs = enqueued_jobs.select { |j| j[:job] == DigestRunJob }
        assert_equal 1, run_jobs.size

        # First arg is digest id, second is the OLD occurrence as ISO8601
        args = run_jobs.first[:args]
        assert_equal digest.id, args.first
        assert_equal old_run.iso8601, args.second

        # Schedule should have been advanced
        assert digest.reload.next_run_at > Time.current
      end
    end
  end

  test "skips disabled digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      build_digest(next_run_at: 1.hour.ago, enabled: false)
      DigestSweepJob.perform_now
      assert_no_enqueued_jobs(only: DigestRunJob)
    end
  end

  test "skips future digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      build_digest(next_run_at: 1.hour.from_now)
      DigestSweepJob.perform_now
      assert_no_enqueued_jobs(only: DigestRunJob)
    end
  end
end
