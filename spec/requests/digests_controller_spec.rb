# frozen_string_literal: true

require "rails_helper"

# Integration tests for DigestsController. All routes live behind
# require_digests_enabled (ENABLE_DIGESTS=1) and require authentication.
#
# Entitlements: the default workspace plan is "free", which blocks digests
# (allowed: false). Happy-path create/update tests add the entitlement_override
# so the guard passes without touching the feature flag separately.
RSpec.describe "Digests", type: :request do
  include ActiveJob::TestHelper

  before do
    @workspace = Workspace.create!(name: "Digest Ctrl WS-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Dana",
      email_address: "dana-ctrl-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)
  end

  # ── Feature gate ─────────────────────────────────────────────────────────────

  it "returns 404 for every route when ENABLE_DIGESTS is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      get digests_path
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Index ─────────────────────────────────────────────────────────────────────

  it "GET /digests lists the user's digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get digests_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(digest.name)
    end
  end

  it "GET /digests only shows the current user's digests" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user = @workspace.users.create!(
        name: "Other", email_address: "other-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user, name: "Other User Digest")
      my_digest    = create_digest(name: "My Digest")

      get digests_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(my_digest.name)
      expect(response.body).not_to include(other_digest.name)
    end
  end

  # ── New (gallery) ──────────────────────────────────────────────────────────────

  it "GET /digests/new without preset renders the preset gallery" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path
      expect(response).to have_http_status(:ok)
    end
  end

  it "GET /digests/new?preset=week_ahead pre-fills the form with the preset" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path(preset: "week_ahead")
      expect(response).to have_http_status(:ok)
      # The form is pre-filled with the preset label
      expect(response.body).to include(I18n.t("digests.presets.week_ahead.label"))
    end
  end

  it "GET /digests/new?preset=bogus returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      get new_digest_path(preset: "bogus_key")
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Show ──────────────────────────────────────────────────────────────────────

  it "GET /digests/:id shows the digest" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get digest_path(digest)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(digest.name)
    end
  end

  it "GET /digests/:id for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Else", email_address: "else-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)
      get digest_path(other_digest)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Edit ──────────────────────────────────────────────────────────────────────

  it "GET /digests/:id/edit renders the edit form" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      get edit_digest_path(digest)
      expect(response).to have_http_status(:ok)
    end
  end

  # ── Create ────────────────────────────────────────────────────────────────────

  it "POST /digests creates a digest when entitlement allows" do
    with_env("ENABLE_DIGESTS" => "1") do
      allow_digests!
      expect {
        post digests_path, params: digest_params
      }.to change(ScheduledDigest, :count).by(1)

      expect(response).to have_http_status(:found)
    end
  end

  it "POST /digests is blocked when the entitlement is denied" do
    with_env("ENABLE_DIGESTS" => "1") do
      # Default free plan denies digests — no override needed.
      post digests_path, params: digest_params
      # EntitlementGuard redirects back with a warning flash.
      expect(response).to have_http_status(:found)
      expect(ScheduledDigest.count).to eq(0)
    end
  end

  # ── Config assembly & normalization ───────────────────────────────────────────

  it "POST /digests normalizes source config: window_days is an Integer, include_overdue is a boolean" do
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

      digest   = ScheduledDigest.last
      task_src = digest.config["sources"].find { |s| s["type"] == "tasks" }
      expect(task_src).not_to be_nil
      expect(task_src["window_days"]).to eq(14), "window_days must be an Integer"
      expect(task_src["include_overdue"]).to eq(true), "include_overdue must be true boolean"
    end
  end

  it "POST /digests drops unchecked sources from config" do
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
          tasks_window_days: "7"  # sent but no source_tasks checkbox
        }
      }

      digest = ScheduledDigest.last
      types  = digest.config["sources"].map { |s| s["type"] }
      expect(types).to include("emails")
      expect(types).not_to include("tasks"), "unchecked source must be dropped"
    end
  end

  # ── Update ────────────────────────────────────────────────────────────────────

  it "PATCH /digests/:id updates the digest and redirects" do
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

      expect(digest.reload.name).to eq("Updated name")
    end
  end

  # ── Destroy ───────────────────────────────────────────────────────────────────

  it "DELETE /digests/:id destroys the digest and redirects to the list" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest
      expect {
        delete digest_path(digest)
      }.to change(ScheduledDigest, :count).by(-1)

      expect(response).to redirect_to(digests_path)
    end
  end

  it "DELETE /digests/:id for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Intruder", email_address: "intruder-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)

      expect {
        delete digest_path(other_digest)
      }.not_to change(ScheduledDigest, :count)

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Run now ───────────────────────────────────────────────────────────────────

  it "POST /digests/:id/run_now enqueues DigestRunJob with manual: true" do
    with_env("ENABLE_DIGESTS" => "1") do
      digest = create_digest

      expect {
        post run_now_digest_path(digest)
      }.to have_enqueued_job(DigestRunJob)

      job    = enqueued_jobs.find { |j| j[:job] == DigestRunJob }
      args   = job[:args]
      expect(args[0]).to eq(digest.id)
      # args last element is the serialized kwargs hash
      kwargs = args.last
      expect(kwargs).to be_a(Hash)
      expect(kwargs["manual"]).to eq(true)
    end
  end

  it "POST /digests/:id/run_now for another user's digest returns 404" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Hacker", email_address: "hacker-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = create_digest(user: other_user)

      expect {
        post run_now_digest_path(other_digest)
      }.not_to have_enqueued_job(DigestRunJob)

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Authentication ────────────────────────────────────────────────────────────

  it "GET /digests redirects to sign-in when not authenticated" do
    with_env("ENABLE_DIGESTS" => "1") do
      delete session_path
      get digests_path
      expect(response).to have_http_status(:found)
    end
  end

  private

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
