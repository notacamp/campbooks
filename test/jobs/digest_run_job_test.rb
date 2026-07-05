# frozen_string_literal: true

require "test_helper"

class DigestRunJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  setup do
    @ws     = Workspace.create!(name: "Run WS")
    @user   = @ws.users.create!(name: "Runner", email_address: "runner@example.com", password: "password123")
    @digest = @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Run digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] },
      ai_enabled:  false,
      deliver_by_email: false,
      show_in_feed: false
    )
  end

  test "no-op when feature flag is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      assert_no_difference "DigestIssue.count" do
        DigestRunJob.perform_now(@digest.id, Time.current.iso8601)
      end
    end
  end

  test "no-op when digest does not exist" do
    with_env("ENABLE_DIGESTS" => "1") do
      assert_no_difference "DigestIssue.count" do
        DigestRunJob.perform_now(SecureRandom.uuid, Time.current.iso8601)
      end
    end
  end

  test "no-op when digest is disabled (non-manual)" do
    with_env("ENABLE_DIGESTS" => "1") do
      @digest.update!(enabled: false)
      assert_no_difference "DigestIssue.count" do
        DigestRunJob.perform_now(@digest.id, Time.current.iso8601)
      end
    end
  end

  test "manual run proceeds even when digest is disabled" do
    with_env("ENABLE_DIGESTS" => "1") do
      @ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      @digest.update!(enabled: false)
      assert_difference "DigestIssue.count", 1 do
        DigestRunJob.perform_now(@digest.id, Time.current.iso8601, manual: true)
      end
    end
  end

  test "clears Current.workspace and Current.acting_user after run" do
    with_env("ENABLE_DIGESTS" => "1") do
      @ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      DigestRunJob.perform_now(@digest.id, Time.current.iso8601)
      assert_nil Current.workspace,   "Current.workspace must be cleared after job"
      assert_nil Current.acting_user, "Current.acting_user must be cleared after job"
    end
  end

  test "gates on workspace entitlement" do
    with_env("ENABLE_DIGESTS" => "1") do
      @ws.update!(entitlement_overrides: { "digests" => { "allowed" => false } })
      assert_no_difference "DigestIssue.count" do
        DigestRunJob.perform_now(@digest.id, Time.current.iso8601)
      end
    end
  end

  test "generates an issue when all gates pass" do
    with_env("ENABLE_DIGESTS" => "1") do
      @ws.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
      assert_difference "DigestIssue.count", 1 do
        DigestRunJob.perform_now(@digest.id, Time.current.iso8601)
      end
    end
  end
end
