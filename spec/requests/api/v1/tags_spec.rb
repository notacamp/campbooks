require "rails_helper"

RSpec.describe "API v1 tags", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  describe "GET /api/v1/tags" do
    it "lists workspace tags" do
      Tag.create!(workspace: workspace, name: "Invoices", color: "#ccc", source: :local)

      get api_v1_tags_path, headers: api_auth_headers(workspace: workspace, user: user, scopes: "tags:read")

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"].map { |t| t["name"] }).to include("Invoices")
    end
  end

  describe "POST /api/v1/emails/:email_id/tags" do
    it "adds a tag by id" do
      email = create(:email_message, email_account: account)
      tag = Tag.create!(workspace: workspace, name: "Invoices", color: "#ccc", source: :local)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "tags:write")

      post api_v1_email_tags_path(email), params: { tag_id: tag.id }, headers: headers

      expect(response).to have_http_status(:created)
      expect(email.reload.tags).to include(tag)
    end

    it "adds a tag by (case-insensitive) name" do
      email = create(:email_message, email_account: account)
      Tag.create!(workspace: workspace, name: "Receipts", color: "#ccc", source: :local)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "tags:write")

      post api_v1_email_tags_path(email), params: { name: "receipts" }, headers: headers

      expect(response).to have_http_status(:created)
      expect(email.reload.tags.map(&:name)).to include("Receipts")
    end

    it "404s for an email the user can't read" do
      other_account = create(:email_account, workspace: workspace) # no EmailAccountUser for user
      email = create(:email_message, email_account: other_account)
      tag = Tag.create!(workspace: workspace, name: "Invoices", color: "#ccc", source: :local)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "tags:write")

      post api_v1_email_tags_path(email), params: { tag_id: tag.id }, headers: headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /api/v1/emails/:email_id/tags/:id" do
    it "removes a tag from the email" do
      email = create(:email_message, email_account: account)
      tag = Tag.create!(workspace: workspace, name: "Invoices", color: "#ccc", source: :local)
      email.tags << tag
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "tags:write")

      delete api_v1_email_tag_path(email, tag), headers: headers

      expect(response).to have_http_status(:no_content)
      expect(email.reload.tags).to be_empty
    end
  end
end
