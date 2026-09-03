require "rails_helper"

RSpec.describe "API v1 bulk email actions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    allow(Emails::InboxBroadcaster).to receive(:remove)
    allow(Emails::InboxBroadcaster).to receive(:upsert)
    allow(Emails::InboxBroadcaster).to receive(:replace)
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "emails:write")
  end

  def path_for(name)
    api_v1_email_bulk_path(name: name)
  end

  describe "POST /api/v1/emails/bulk/:name" do
    it "dispatches through Emails::BulkActions and returns the affected ids" do
      e1 = create(:email_message, email_account: account)
      e2 = create(:email_message, email_account: account)

      result = Emails::BulkActions::Result.new(
        tool: "archive", result: { archived_count: 2 },
        selected_ids: [ e1.id.to_s, e2.id.to_s ], all_ids: [ e1.id.to_s, e2.id.to_s ]
      )
      expect(Emails::BulkActions).to receive(:call)
        .with(hash_including(tool: "archive", user: user))
        .and_return(result)

      post path_for("archive"), params: { email_ids: [ e1.id, e2.id ] }, headers: write_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body.dig("data", "action")).to eq("archive")
      expect(body.dig("data", "result", "archived_count")).to eq(2)
      expect(body.dig("data", "ids")).to match_array([ e1.id.to_s, e2.id.to_s ])
    end

    it "really archives the selection end to end" do
      email = create(:email_message, email_account: account)

      post path_for("archive"), params: { email_ids: [ email.id ] }, headers: write_headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "result", "archived_count")).to eq(1)
    end

    it "422s an empty selection" do
      post path_for("archive"), params: { email_ids: [] }, headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("no_emails_selected")
    end

    it "rejects an unknown bulk action with 422 and no dispatch" do
      expect(Emails::BulkActions).not_to receive(:call)

      post path_for("nuke"), params: { email_ids: [ 1 ] }, headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_action")
    end

    it "surfaces a tool's soft error (tag with no name) as action_failed" do
      email = create(:email_message, email_account: account)
      result = Emails::BulkActions::Result.new(
        tool: "tag", result: { error: "Tag name required" },
        selected_ids: [ email.id.to_s ], all_ids: [ email.id.to_s ]
      )
      allow(Emails::BulkActions).to receive(:call).and_return(result)
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "tags:write")

      post path_for("tag"), params: { email_ids: [ email.id ] }, headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "message")).to eq("Tag name required")
    end

    describe "scope enforcement" do
      it "403s archive without emails:write" do
        headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")

        post path_for("archive"), params: { email_ids: [ 1 ] }, headers: headers

        expect(response).to have_http_status(:forbidden)
      end

      it "403s tag without tags:write" do
        post path_for("tag"), params: { email_ids: [ 1 ] }, headers: write_headers # emails:write only

        expect(response).to have_http_status(:forbidden)
      end
    end
  end
end
