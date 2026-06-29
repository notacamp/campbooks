require "rails_helper"

RSpec.describe "Files internal documents", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  before { sign_in(user) }

  it "publishes internal_document.created on create" do
    expect do
      post written_documents_path, params: { authored_document: { title: "Brief", html_content: "<p>Hi</p>" } }
    end.to change { workspace.events.where(name: "internal_document.created").count }.by(1)
  end

  it "publishes internal_document.updated on update" do
    doc = create(:authored_document, workspace: workspace)
    expect do
      patch written_document_path(doc), params: { authored_document: { title: "New title" } }
    end.to change { workspace.events.where(name: "internal_document.updated").count }.by(1)
  end
end
