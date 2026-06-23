require "rails_helper"

RSpec.describe "API v1 document types", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  it "lists the workspace's document types" do
    DocumentType.create!(workspace: workspace, name: "receipt", color: "#000", prompt: "x")

    get api_v1_document_types_path,
        headers: api_auth_headers(workspace: workspace, user: user, scopes: "document_types:read")

    expect(response).to have_http_status(:ok)
    expect(response.parsed_body["data"].map { |t| t["name"] }).to include("receipt")
  end

  it "does not leak another workspace's types" do
    DocumentType.create!(workspace: create(:workspace), name: "receipt", color: "#000", prompt: "x")

    get api_v1_document_types_path,
        headers: api_auth_headers(workspace: workspace, user: user, scopes: "document_types:read")

    expect(response.parsed_body["data"]).to be_empty
  end

  it "403s without the document_types:read scope" do
    get api_v1_document_types_path,
        headers: api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")

    expect(response).to have_http_status(:forbidden)
  end
end
