require "rails_helper"

RSpec.describe Settings::MembersController, type: :controller do
  render_views

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    session_record = create(:session, user: user)
    cookies.signed[:session_id] = session_record.id
    Current.workspace = workspace
  end

  describe "GET index" do
    it "lists members of the current workspace" do
      other = create(:user, name: "Alice", workspace: workspace)
      get :index
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(user.name)
      expect(response.body).to include("Alice")
    end

    it "shows pending invitations" do
      create(:invitation, workspace: workspace, email: "pending@example.com",
             invited_by: user, status: :pending)
      get :index
      expect(response.body).to include("pending@example.com")
    end
  end

  describe "PATCH update (workspace role management)" do
    let!(:teammate) { create(:user, workspace: workspace, name: "Tessa") }

    it "lets a workspace admin promote a teammate" do
      user.update!(role: :admin)

      patch :update, params: { id: teammate.id, role: "admin" }

      expect(teammate.reload).to be_admin
      expect(flash[:success]).to be_present
    end

    it "denies a plain member" do
      patch :update, params: { id: teammate.id, role: "admin" }

      expect(teammate.reload).not_to be_admin
      expect(flash[:error]).to be_present
    end

    it "refuses a self role change" do
      user.update!(role: :admin)

      patch :update, params: { id: user.id, role: "member" }

      expect(user.reload).to be_admin
      expect(flash[:error]).to be_present
    end

    it "rejects unknown roles" do
      user.update!(role: :admin)

      patch :update, params: { id: teammate.id, role: "overlord" }

      expect(teammate.reload).not_to be_admin
      expect(flash[:error]).to be_present
    end
  end
end
