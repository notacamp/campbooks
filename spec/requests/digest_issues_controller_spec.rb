# frozen_string_literal: true

require "rails_helper"

# Integration tests for DigestIssuesController. The single action (#show)
# is scoped through the current user's digest so a mismatched pair 404s per
# the invisible-resource convention.
RSpec.describe "DigestIssues", type: :request do
  before do
    @workspace = Workspace.create!(name: "Issue Ctrl WS-#{SecureRandom.hex(4)}")
    @user = @workspace.users.create!(
      name: "Isadora",
      email_address: "isadora-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
    sign_in(@user)

    @digest = @workspace.scheduled_digests.create!(
      user:        @user,
      name:        "Issue digest",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )

    @issue = @digest.issues.create!(
      workspace_id: @workspace.id,
      user_id:      @user.id,
      period_start: 1.week.ago,
      period_end:   Time.current,
      status:       :generated,
      content: {
        "overview"  => "Nothing urgent this week.",
        "sections"  => [
          { "key" => "emails", "items" => [
            { "source_type" => "email", "source_id" => SecureRandom.uuid,
              "title" => "Invoice #1", "subtitle" => "billing@acme.example",
              "timestamp" => 2.days.ago.iso8601 }
          ] }
        ],
        "meta" => { "list_mode" => false, "counts" => { "emails" => 1 }, "source_errors" => [] }
      }
    )
  end

  # ── Feature gate ─────────────────────────────────────────────────────────────

  it "returns 404 when ENABLE_DIGESTS is off" do
    with_env("ENABLE_DIGESTS" => nil) do
      get digest_issue_path(@digest, @issue)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Happy path ───────────────────────────────────────────────────────────────

  it "GET /digests/:digest_id/issues/:id shows the issue" do
    with_env("ENABLE_DIGESTS" => "1") do
      get digest_issue_path(@digest, @issue)
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Nothing urgent this week.")
      expect(response.body).to include("Invoice #1")
    end
  end

  # ── Scoping ──────────────────────────────────────────────────────────────────

  it "returns 404 when the digest belongs to another user" do
    with_env("ENABLE_DIGESTS" => "1") do
      other_user   = @workspace.users.create!(
        name: "Eavesdropper", email_address: "evs-#{SecureRandom.hex(4)}@example.com", password: "password123"
      )
      other_digest = @workspace.scheduled_digests.create!(
        user:        other_user,
        name:        "Other digest",
        rrule:       "FREQ=WEEKLY",
        next_run_at: 1.week.from_now,
        config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
      )

      # Try to read my issue through the other user's digest_id
      get digest_issue_path(other_digest, @issue)
      expect(response).to have_http_status(:not_found)
    end
  end

  it "returns 404 when the issue does not belong to the given digest" do
    with_env("ENABLE_DIGESTS" => "1") do
      second_digest = @workspace.scheduled_digests.create!(
        user:        @user,
        name:        "Second digest",
        rrule:       "FREQ=WEEKLY",
        next_run_at: 1.week.from_now,
        config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
      )

      # @issue belongs to @digest, not second_digest
      get digest_issue_path(second_digest, @issue)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Authentication ────────────────────────────────────────────────────────────

  it "redirects to sign-in when not authenticated" do
    with_env("ENABLE_DIGESTS" => "1") do
      delete session_path
      get digest_issue_path(@digest, @issue)
      expect(response).to have_http_status(:found)
    end
  end
end
