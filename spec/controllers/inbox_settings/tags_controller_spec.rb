require "rails_helper"

RSpec.describe InboxSettings::TagsController, type: :controller do
  render_views

  let(:user) { create(:user) }
  let(:workspace) { user.workspace }

  before do
    session_record = create(:session, user: user)
    allow(controller).to receive(:find_session_by_cookie).and_return(session_record)
    allow(controller).to receive(:redirect_to_onboarding_if_incomplete)
  end

  describe "GET index" do
    it "lists the workspace tags" do
      workspace.tags.create!(name: "Invoices", color: "#3b82f6", source: :local)

      get :index
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Invoices")
    end
  end

  describe "POST create" do
    it "creates a tag with an AI prompt and renders its row" do
      expect {
        post :create, params: { tag: { name: "Receipts", color: "#10b981", prompt: "Purchase receipts" } }, format: :turbo_stream
      }.to change(workspace.tags, :count).by(1)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Receipts")
      expect(workspace.tags.find_by(name: "Receipts").prompt).to eq("Purchase receipts")
    end

    it "re-renders the form with an error when invalid" do
      post :create, params: { tag: { name: "", color: "#10b981" } }, format: :turbo_stream
      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "DELETE destroy" do
    it "deletes the tag" do
      tag = workspace.tags.create!(name: "Temp", color: "#999999", source: :local)

      expect {
        delete :destroy, params: { id: tag.id }, format: :turbo_stream
      }.to change(workspace.tags, :count).by(-1)
    end

    it "refuses to delete the security_flagged tag" do
      tag = workspace.tags.create!(name: "security_flagged", color: "#dc2626", source: :local)

      expect {
        delete :destroy, params: { id: tag.id }, format: :turbo_stream
      }.not_to change(workspace.tags, :count)
    end
  end
end
