# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app email accounts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true,
                                can_manage: true, owner: true)
  end

  def headers = api_app_headers(user)

  describe "GET /api/app/email_accounts" do
    it "returns 200 with the user's connected accounts" do
      get "/api/app/email_accounts", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to be_an(Array)
      expect(data.first["id"]).to eq(account.id)
      expect(data.first).to include("email_address", "provider", "can_read", "is_owner")
    end

    it "401s without a token" do
      get "/api/app/email_accounts"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/email_accounts/:id" do
    it "returns 200 when manager updates the display name" do
      patch "/api/app/email_accounts/#{account.id}",
            params: { name: "My Work Inbox" },
            headers: headers

      expect(response).to have_http_status(:ok)
      expect(account.reload.name).to eq("My Work Inbox")
    end

    it "returns 403 when a non-manager tries to update" do
      reader = create(:user, workspace: workspace)
      create(:email_account_user, user: reader, email_account: account, can_read: true,
                                  can_manage: false, owner: false)

      patch "/api/app/email_accounts/#{account.id}",
            params: { name: "Hacked" },
            headers: api_app_headers(reader)

      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "DELETE /api/app/email_accounts/:id" do
    before do
      allow(EmailAccountRemovalJob).to receive(:perform_later)
    end

    it "returns 204 when owner disconnects" do
      delete "/api/app/email_accounts/#{account.id}", headers: headers

      expect(response).to have_http_status(:no_content)
      expect(account.reload.active).to be false
      expect(EmailAccountRemovalJob).to have_received(:perform_later).with(account.id)
    end

    it "returns 404 when non-owner tries to disconnect (404-not-403 rule)" do
      reader = create(:user, workspace: workspace)
      create(:email_account_user, user: reader, email_account: account, can_read: true,
                                  can_manage: true, owner: false)

      delete "/api/app/email_accounts/#{account.id}", headers: api_app_headers(reader)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "GET /api/app/email_accounts/:id/sharing" do
    it "returns 200 with sharing data for the owner" do
      get "/api/app/email_accounts/#{account.id}/sharing", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to have_key("account")
      expect(data["members"]).to be_an(Array)
    end

    it "404s for a non-owner (404-not-403 rule)" do
      reader = create(:user, workspace: workspace)
      create(:email_account_user, user: reader, email_account: account, can_read: true, owner: false)

      get "/api/app/email_accounts/#{account.id}/sharing", headers: api_app_headers(reader)
      expect(response).to have_http_status(:not_found)
    end
  end
end
