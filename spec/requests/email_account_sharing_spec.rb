require "rails_helper"

RSpec.describe "EmailAccount sharing", type: :request do
  let(:workspace) { create(:workspace) }
  let(:owner) { create(:user, workspace: workspace, name: "Olivia Owner") }
  let!(:teammate) { create(:user, workspace: workspace, name: "Tessa Teammate") }
  let(:account) { create(:email_account, workspace: workspace, email_address: "shared@acme.com") }

  before { create(:email_account_user, :owner, user: owner, email_account: account) }

  describe "GET /email_accounts/:id/sharing" do
    it "renders the sharing panel for the owner (and the component)" do
      sign_in(owner)
      get sharing_email_account_path(account)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Manage access")
      expect(response.body).to include("Olivia Owner")
      expect(response.body).to include("Tessa Teammate") # an addable workspace member
    end

    it "blocks a non-owner sharee from opening it" do
      create(:email_account_user, :viewer, user: teammate, email_account: account)
      sign_in(teammate)
      get sharing_email_account_path(account)

      expect(response).to have_http_status(:redirect)
    end
  end

  describe "PATCH /email_accounts/:id (sharing updates)" do
    it "adds a workspace member with the chosen role" do
      sign_in(owner)

      expect {
        patch email_account_path(account), params: { user_email: teammate.email_address, role: "collaborator" }
      }.to change { account.email_account_users.count }.by(1)

      expect(account.email_account_users.find_by(user: teammate).role).to eq("collaborator")
      expect(response).to redirect_to(sharing_email_account_path(account))
    end

    it "changes an existing member's role" do
      entry = create(:email_account_user, :viewer, user: teammate, email_account: account)
      sign_in(owner)

      patch email_account_path(account), params: { user_email: teammate.email_address, role: "manager" }

      expect(entry.reload.role).to eq("manager")
    end

    it "removes a member" do
      create(:email_account_user, :viewer, user: teammate, email_account: account)
      sign_in(owner)

      expect {
        patch email_account_path(account), params: { user_email: teammate.email_address, remove: "true" }
      }.to change { account.email_account_users.count }.by(-1)
    end

    it "rejects an unknown role without creating an entry" do
      sign_in(owner)

      patch email_account_path(account), params: { user_email: teammate.email_address, role: "superadmin" }

      expect(account.email_account_users.find_by(user: teammate)).to be_nil
    end

    it "won't let a non-owner change who has access" do
      create(:email_account_user, :collaborator, user: teammate, email_account: account)
      sign_in(teammate)

      patch email_account_path(account), params: { user_email: owner.email_address, role: "viewer" }

      expect(account.email_account_users.find_by(user: owner).role).to eq("owner")
    end
  end
end
