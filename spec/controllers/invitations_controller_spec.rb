require "rails_helper"

RSpec.describe InvitationsController, type: :controller do
  render_views

  let(:workspace) { create(:workspace) }
  let(:inviter) { create(:user, workspace: workspace) }
  let(:invitation) { create(:invitation, workspace: workspace, invited_by: inviter) }

  def authenticate(user)
    session_record = create(:session, user: user)
    # Controller specs carry no signed cookie, so resume_session can't find the
    # session on its own — hand it the record directly (mirrors the contacts spec).
    allow(controller).to receive(:find_session_by_cookie).and_return(session_record)
  end

  describe "GET show" do
    it "renders acceptance page for authenticated user from different org" do
      other_org = create(:workspace)
      other_user = create(:user, workspace: other_org)
      authenticate(other_user)

      get :show, params: { token: invitation.token }
      expect(response).to have_http_status(:ok)
      expect(CGI.unescapeHTML(response.body)).to include("You're invited!")
      expect(response.body).to include(workspace.name)
    end

    it "redirects authenticated user already in the org" do
      authenticate(inviter)

      get :show, params: { token: invitation.token }
      expect(response).to redirect_to(root_path)
      expect(flash[:success]).to include("already a member")
    end

    it "stores token and redirects to registration for unauthenticated users" do
      get :show, params: { token: invitation.token }
      expect(session[:invitation_token]).to eq(invitation.token)
      expect(response).to redirect_to(new_registration_path)
    end

    it "redirects for expired invitation" do
      invitation.update!(status: :pending, expires_at: 1.day.ago)
      get :show, params: { token: invitation.token }
      expect(response).to redirect_to(root_path)
      expect(flash[:error]).to include("expired")
    end

    it "redirects for cancelled invitation" do
      invitation.cancel!
      get :show, params: { token: invitation.token }
      expect(response).to redirect_to(root_path)
      expect(flash[:error]).to include("cancelled")
    end

    it "redirects for accepted invitation" do
      accepted_user = create(:user, workspace: workspace)
      invitation.accept!(accepted_user)
      get :show, params: { token: invitation.token }
      expect(response).to redirect_to(root_path)
      expect(flash[:error]).to include("already been accepted")
    end
  end

  describe "POST accept" do
    it "accepts invitation for authenticated user" do
      other_org = create(:workspace)
      new_user = create(:user, workspace: other_org)
      authenticate(new_user)

      post :accept, params: { token: invitation.token }

      new_user.reload
      expect(new_user.workspace).to eq(workspace)
      expect(invitation.reload).to be_accepted
      expect(response).to redirect_to(root_path)
      expect(flash[:success]).to include("You have joined")
    end

    it "redirects unauthenticated user to registration" do
      post :accept, params: { token: invitation.token }
      expect(session[:invitation_token]).to eq(invitation.token)
      expect(response).to redirect_to(new_registration_path)
    end
  end
end
