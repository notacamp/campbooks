require "rails_helper"

RSpec.describe Settings::InvitationsController, type: :controller do
  render_views

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    session_record = create(:session, user: user)
    cookies.signed[:session_id] = session_record.id
    Current.workspace = workspace
  end

  describe "POST create" do
    it "creates an invitation and sends email" do
      expect {
        post :create, params: { invitation: { email: "new@example.com" } }
      }.to change(Invitation, :count).by(1)

      expect(response).to redirect_to(settings_members_path)
      expect(flash[:success]).to include("new@example.com")
    end

    it "rejects invalid email" do
      post :create, params: { invitation: { email: "invalid" } }
      expect(response).to redirect_to(settings_members_path)
      expect(flash[:error]).to be_present
    end

    it "rejects email of existing member" do
      create(:user, email_address: "member@example.com", workspace: workspace)
      post :create, params: { invitation: { email: "member@example.com" } }
      expect(response).to redirect_to(settings_members_path)
      expect(flash[:error]).to include("already a member")
    end
  end

  describe "DELETE destroy" do
    it "cancels the invitation" do
      invitation = create(:invitation, workspace: workspace, invited_by: user)
      delete :destroy, params: { id: invitation.id }
      expect(invitation.reload).to be_cancelled
    end
  end

  describe "POST resend" do
    it "regenerates token and resends email" do
      invitation = create(:invitation, :cancelled, workspace: workspace, invited_by: user)
      old_token = invitation.token

      post :resend, params: { id: invitation.id }

      invitation.reload
      expect(invitation.token).not_to eq(old_token)
      expect(invitation).to be_pending
      expect(response).to redirect_to(settings_members_path)
    end
  end
end
