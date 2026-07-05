# frozen_string_literal: true

require "test_helper"

# Integration tests for DigestsController. All routes live behind
# require_digests_enabled (ENABLE_DIGESTS=1) and require authentication.
#
# Entitlements: the default workspace plan is "free", which blocks digests
# (allowed: false). Happy-path create/update tests add the entitlement_override
# so the guard passes without touching the feature flag separately.
class DigestsControllerTest < ActionDispatch::IntegrationTest
  include ActiveJob::TestHelper

  setup do
    @workspace = Workspace.create!(name: "Digest Ctrl WS-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Dana",
      email_address: "dana-ctrl-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)
  end

  # ── Feature gate ─────────────────────────────────────────────────────────────

  test "returns 404 for every route when ENABLE_DIGESTS is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      get digests_path
      assert_response :not_found
    end
  end

  # ── Index ─────────────────────────────────────────────────────────────────────

  test "GET /digests lists the user's digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get digests_path
      assert_response :success
      assert_includes response.body, digest.name
    end
  end

  test "GET /digests only shows the current user's digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user = @workspace.users.create!(
        name: "Other", email_address: "other-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user, name: "Other User Digest")
      my_digest    = create_digest(name: "My Digest")

      get digests_path
      assert_response :success
      assert_includes response.body, my_digest.name
      assert_not_includes response.body, other_digest.name
    end
  end

  # ── New (gallery) ──────────────────────────────────────────────────────────────

  test "GET /digests/new without preset renders the preset gallery" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path
      assert_response :success
    end
  end

  test "GET /digests/new?preset=week_ahead pre-fills the form with the preset" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path(preset: "week_ahead")
      assert_response :success
      # The form is pre-filled with the preset label
      assert_includes response.body, I18n.t("digests.presets.week_ahead.label")
    end
  end

  test "GET /digests/new?preset=bogus returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path(preset: "bogus_key")
      assert_response :not_found
    end
  end

  # ── Show ──────────────────────────────────────────────────────────────────────

  test "GET /digests/:id shows the digest" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get digest_path(digest)
      assert_response :success
      assert_includes response.body, digest.name
    end
  end

  test "GET /digests/:id for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Else", email_address: "else-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)
      get digest_path(other_digest)
      assert_response :not_found
    end
  end

  # ── Edit ──────────────────────────────────────────────────────────────────────

  test "GET /digests/:id/edit renders the edit form" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get edit_digest_path(digest)
      assert_response :success
    end
  end

  # ── Create ────────────────────────────────────────────────────────────────────

  test "POST /digests creates a digest when entitlement allows" do
    with_env("ENABLE_DIGESTS" => "1") do
      allow_digests!
      assert_difference -> { ScheduledDigest.count }, 1 do
        post digests_path, params: digest_params
      end
      assert_response :redirect
    end
  end

  test "POST /digests is blocked when the entitlement is denied" do
    with_env("ENABLE_DIGESTS" => "1") do
      # Default free plan denies digests — no override needed.
      post digests_path, params: digest_params
      # EntitlementGuard redirects back with a warning flash.
      assert_response :redirect
      assert_equal 0, ScheduledDigest.count
    end
  end

  # ── Config assembly & normalization ───────────────────────────────────────────

  test "POST /digests normalizes source config: window_days is an Integer, include_overdue is a boolean" do
    with_env("ENABLE_DIGESTS" => "1") do
      allow_digests!

      post digests_path, params: {
        digest: {
          name: "Config test",
          rrule: "FREQ=WEEKLY",
          first_run_at: 1.week.from_now.iso8601,
          ai_enabled: "0",
          deliver_by_email: "0",
          show_in_feed: "0",
          source_tasks: "1",
          tasks_window_days: "14",
          tasks_include_overdue: "1"
        }
      }

      digest = ScheduledDigest.last
      task_src = digest.config["sources"].find { |s| s["type"] == "tasks" }
      assert_not_nil task_src
      assert_equal 14, task_src["window_days"], "window_days must be an Integer"
      assert_equal true, task_src["include_overdue"], "include_overdue must be true boolean"
    end
  end

  test "POST /digests drops unchecked sources from config" do
    with_env("ENABLE_DIGESTS" => "1") do
      allow_digests!

      # Only source_emails checked; source_tasks NOT checked but params sent.
      post digests_path, params: {
        digest: {
          name: "Emails only",
          rrule: "FREQ=WEEKLY",
          first_run_at: 1.week.from_now.iso8601,
          ai_enabled: "0",
          deliver_by_email: "0",
          show_in_feed: "0",
          source_emails: "1",
          emails_query: "",
          tasks_window_days: "7"   # sent but no source_tasks checkbox
        }
      }

      digest = ScheduledDigest.last
      types = digest.config["sources"].map { |s| s["type"] }
      assert_includes types, "emails"
      assert_not_includes types, "tasks", "unchecked source must be dropped"
    end
  end

  # ── Update ────────────────────────────────────────────────────────────────────

  test "PATCH /digests/:id updates the digest and redirects" do
    with_env("ENABLE_DIGESTS" => "1") do
      allow_digests!
      digest = create_digest

      patch digest_path(digest), params: {
        digest: {
          name: "Updated name",
          rrule: "FREQ=DAILY",
          ai_enabled: "0",
          deliver_by_email: "0",
          show_in_feed: "0",
          source_emails: "1",
          emails_query: ""
        }
      }

      assert_equal "Updated name", digest.reload.name
    end
  end

  # ── Destroy ───────────────────────────────────────────────────────────────────

  test "DELETE /digests/:id destroys the digest and redirects to the list" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      assert_difference -> { ScheduledDigest.count }, -1 do
        delete digest_path(digest)
      end
      assert_redirected_to digests_path
    end
  end

  test "DELETE /digests/:id for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Intruder", email_address: "intruder-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)

      assert_no_difference -> { ScheduledDigest.count } do
        delete digest_path(other_digest)
      end
      assert_response :not_found
    end
  end

  # ── Run now ───────────────────────────────────────────────────────────────────

  test "POST /digests/:id/run_now enqueues DigestRunJob with manual: true" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest

      assert_enqueued_with(job: DigestRunJob) do
        post run_now_digest_path(digest)
      end

      job = enqueued_jobs.find { |j| j[:job] == DigestRunJob }
      # Third positional arg is the keyword hash — check manual: true
      args = job[:args]
      assert_equal digest.id, args[0]
      # args[2] is the serialized kwargs hash
      kwargs = args.last
      assert kwargs.is_a?(Hash), "DigestRunJob must receive kwargs"
      assert_equal true, kwargs["manual"]
    end
  end

  test "POST /digests/:id/run_now for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Hacker", email_address: "hacker-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)

      assert_no_enqueued_jobs(only: DigestRunJob) do
        post run_now_digest_path(other_digest)
      end
      assert_response :not_found
    end
  end

  # ── Authentication ────────────────────────────────────────────────────────────

  test "GET /digests redirects to sign-in when not authenticated" do
    with_env("ENABLE_DIGESTS" => "1") do
      delete session_path
      get digests_path
      assert_response :redirect
    end
  end

  private

  def sign_in(user)
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  def allow_digests!
    @workspace.update!(entitlement_overrides: { "digests" => { "allowed" => true, "enabled" => true } })
  end

  def create_digest(user: @user, name: "Weekly roundup")
    @workspace.scheduled_digests.create!(
      user:        user,
      name:        name,
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  def digest_params
    {
      digest: {
        name: "Weekly roundup",
        rrule: "FREQ=WEEKLY",
        first_run_at: 1.week.from_now.iso8601,
        ai_enabled: "0",
        deliver_by_email: "0",
        show_in_feed: "0",
        source_emails: "1",
        emails_query: ""
      }
    }
  end
end
