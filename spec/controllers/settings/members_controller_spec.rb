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
end
