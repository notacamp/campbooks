require "rails_helper"

RSpec.describe "API v1 scheduled emails", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "scheduled_emails:read")
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "scheduled_emails:write")
  end

  def schedule_attrs(overrides = {})
    { email_account_id: account.id, to_address: "x@acme.com", subject: "Hi",
      body: "Body", scheduled_at: 1.day.from_now.iso8601 }.merge(overrides)
  end

  describe "GET /api/v1/scheduled_emails" do
    it "lists the workspace's scheduled emails, soonest first" do
      create(:scheduled_email, workspace: workspace, email_account: account, created_by: user,
                               subject: "Later", scheduled_at: 2.days.from_now)
      create(:scheduled_email, workspace: workspace, email_account: account, created_by: user,
                               subject: "Sooner", scheduled_at: 1.hour.from_now)

      get api_v1_scheduled_emails_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].map { |e| e["subject"] }).to eq(%w[Sooner Later])
    end

    it "does not leak another workspace's scheduled emails" do
      other = create(:workspace, plan: "pro")
      create(:scheduled_email, workspace: other, created_by: create(:user, workspace: other),
                               email_account: create(:email_account, workspace: other))

      get api_v1_scheduled_emails_path, headers: read_headers

      expect(response.parsed_body["data"]).to be_empty
    end
  end

  describe "GET /api/v1/scheduled_emails/:id" do
    it "404s across workspaces" do
      other = create(:workspace, plan: "pro")
      rec = create(:scheduled_email, workspace: other, created_by: create(:user, workspace: other),
                                     email_account: create(:email_account, workspace: other))

      get api_v1_scheduled_email_path(rec), headers: read_headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/v1/scheduled_emails" do
    it "schedules an email and stamps next_occurrence_at + created_by" do
      expect do
        post api_v1_scheduled_emails_path, params: schedule_attrs, headers: write_headers
      end.to change(ScheduledEmail, :count).by(1)

      expect(response).to have_http_status(:created)
      rec = ScheduledEmail.last
      expect(rec.created_by).to eq(user)
      expect(rec.workspace).to eq(workspace)
      expect(rec.next_occurrence_at).to be_present
    end

    it "403s when the chosen account is not sendable by the user" do
      foreign = create(:email_account, workspace: create(:workspace))

      post api_v1_scheduled_emails_path, params: schedule_attrs(email_account_id: foreign.id), headers: write_headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("no_sendable_account")
    end

    it "403s with only the read scope" do
      post api_v1_scheduled_emails_path, params: schedule_attrs, headers: read_headers
      expect(response).to have_http_status(:forbidden)
    end

    it "403s (entitlement_required) when the plan lacks email scheduling" do
      free_ws = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free_ws)
      free_account = create(:email_account, workspace: free_ws)
      create(:email_account_user, user: free_user, email_account: free_account, can_read: true, can_send: true)
      headers = api_auth_headers(workspace: free_ws, user: free_user, scopes: "scheduled_emails:write")

      post api_v1_scheduled_emails_path,
           params: { email_account_id: free_account.id, to_address: "x@y.com", subject: "Hi",
                     body: "B", scheduled_at: 1.day.from_now.iso8601 },
           headers: headers

      expect(response).to have_http_status(:forbidden)
      expect(response.parsed_body.dig("error", "code")).to eq("entitlement_required")
    end

    it "422s when required fields are missing" do
      post api_v1_scheduled_emails_path, params: { email_account_id: account.id }, headers: write_headers
      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "DELETE /api/v1/scheduled_emails/:id (cancel)" do
    it "soft-cancels via status" do
      rec = create(:scheduled_email, workspace: workspace, email_account: account, created_by: user)

      delete api_v1_scheduled_email_path(rec), headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(rec.reload).to be_cancelled
    end
  end
end
