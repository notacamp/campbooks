# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app invitations", type: :request do
  let(:workspace) { create(:workspace) }
  let(:inviter) { create(:user, workspace: workspace) }

  describe "POST /api/app/invitations/:token/accept" do
    context "invitation for a new (unregistered) user" do
      let(:invite_email) { "invite-new-#{SecureRandom.hex(6)}@example.com" }
      let(:invitation) do
        create(:invitation,
               workspace: workspace,
               invited_by: inviter,
               email: invite_email,
               admin_approved: true,
               status: :pending)
      end

      it "returns registration_required when no account exists for the invited email" do
        post "/api/app/invitations/#{invitation.token}/accept"

        expect(response).to have_http_status(:ok)
        data = response.parsed_body["data"]
        expect(data["status"]).to eq("registration_required")
        expect(data["invitation_token"]).to eq(invitation.token)
        expect(data["email"]).to eq(invite_email)
      end
    end

    context "invitation for an existing user (not yet authenticated)" do
      let(:existing_email) { "existing-#{SecureRandom.hex(6)}@example.com" }
      let!(:existing_user) { create(:user, email_address: existing_email) }
      let(:invitation) do
        create(:invitation,
               workspace: workspace,
               invited_by: inviter,
               email: existing_email,
               admin_approved: true,
               status: :pending)
      end

      it "returns login_required when the user exists but is not authenticated" do
        post "/api/app/invitations/#{invitation.token}/accept"

        expect(response).to have_http_status(:ok)
        data = response.parsed_body["data"]
        expect(data["status"]).to eq("login_required")
        expect(data["email"]).to eq(existing_email)
      end
    end

    context "authenticated user accepting an invite to a new workspace" do
      let(:other_workspace) { create(:workspace) }
      let(:member_email) { "member-#{SecureRandom.hex(6)}@example.com" }
      let(:member) { create(:user, workspace: other_workspace, email_address: member_email) }
      let(:invitation) do
        create(:invitation,
               workspace: workspace,
               invited_by: inviter,
               email: member_email,
               admin_approved: true,
               status: :pending)
      end

      it "accepts the invitation and returns status accepted" do
        post "/api/app/invitations/#{invitation.token}/accept",
             headers: api_app_headers(member)

        expect(response).to have_http_status(:ok)
        data = response.parsed_body["data"]
        expect(data["status"]).to eq("accepted")
        expect(invitation.reload).to be_accepted
      end
    end

    context "authenticated user who is already a member" do
      let(:member_email) { "member-ws-#{SecureRandom.hex(6)}@example.com" }
      let(:member) { create(:user, workspace: workspace, email_address: member_email) }
      let(:invitation) do
        create(:invitation,
               workspace: workspace,
               invited_by: inviter,
               email: member_email,
               admin_approved: true,
               status: :pending)
      end

      it "returns already_member" do
        post "/api/app/invitations/#{invitation.token}/accept",
             headers: api_app_headers(member)

        expect(response).to have_http_status(:ok)
        expect(response.parsed_body.dig("data", "status")).to eq("already_member")
      end
    end

    context "invalid / expired invitation" do
      it "404s for an unknown token" do
        post "/api/app/invitations/does-not-exist/accept"
        expect(response).to have_http_status(:not_found)
      end

      it "422s for an expired invitation" do
        expired = create(:invitation, :expired,
                         workspace: workspace, invited_by: inviter,
                         email: "exp-#{SecureRandom.hex(6)}@example.com",
                         admin_approved: true)

        post "/api/app/invitations/#{expired.token}/accept"
        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("expired")
      end
    end
  end
end
