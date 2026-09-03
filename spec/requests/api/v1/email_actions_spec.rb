require "rails_helper"

RSpec.describe "API v1 email actions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def write_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "emails:write")
  end

  def path_for(email, name)
    api_v1_email_actions_path(email_id: email.id, name: name)
  end

  describe "POST /api/v1/emails/:id/actions/:name" do
    it "archives the thread through the shared registry and returns the updated email" do
      email = create(:email_message, email_account: account)

      expect(EmailActions).to receive(:run)
        .with("archive", hash_including(email_message: email, user: user))
        .and_return({ success: true, tool: "archive", message: "Archived", result: { archived: true } })

      post path_for(email, "archive"), headers: write_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body.dig("data", "id")).to eq(email.id)
      expect(body.dig("meta", "action")).to eq("archive")
      expect(body.dig("meta", "result", "archived")).to be(true)
    end

    it "forwards the action args through to the registry" do
      email = create(:email_message, email_account: account)

      expect(EmailActions).to receive(:run)
        .with("snooze", hash_including(args: hash_including("snoozed_until" => "2026-08-01T09:00:00Z")))
        .and_return({ success: true, tool: "snooze", message: "Snoozed", result: {} })

      post path_for(email, "snooze"),
           params: { args: { snoozed_until: "2026-08-01T09:00:00Z" } },
           headers: write_headers

      expect(response).to have_http_status(:ok)
    end

    it "rejects an unknown action with 422 and no registry call" do
      email = create(:email_message, email_account: account)

      expect(EmailActions).not_to receive(:run)

      post path_for(email, "nuke_everything"), headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("invalid_action")
    end

    it "surfaces a registry failure as the API error envelope" do
      email = create(:email_message, email_account: account)
      allow(EmailActions).to receive(:run)
        .and_return({ success: false, tool: "archive", message: "boom", result: nil })

      post path_for(email, "archive"), headers: write_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("action_failed")
      expect(response.parsed_body.dig("error", "message")).to eq("boom")
    end

    it "maps a mailbox send-permission failure to 403" do
      email = create(:email_message, email_account: account)
      allow(EmailActions).to receive(:run).and_return(
        { success: false, tool: "forward_email",
          message: I18n.t("email_actions.send_permission_denied"), result: nil }
      )
      headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:send")

      post path_for(email, "forward_email"),
           params: { args: { to_address: "someone@example.com" } }, headers: headers

      expect(response).to have_http_status(:forbidden)
    end

    describe "scope enforcement" do
      it "403s an emails:write action when the token only has emails:read" do
        email = create(:email_message, email_account: account)
        headers = api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")

        post path_for(email, "archive"), headers: headers

        expect(response).to have_http_status(:forbidden)
      end

      it "403s a sender action without contacts:write" do
        email = create(:email_message, email_account: account)

        post path_for(email, "block_sender"), headers: write_headers # emails:write only

        expect(response).to have_http_status(:forbidden)
      end

      it "403s forward without emails:send" do
        email = create(:email_message, email_account: account)

        post path_for(email, "forward_email"), headers: write_headers # emails:write only

        expect(response).to have_http_status(:forbidden)
      end
    end

    it "404s for an email in another workspace (no existence leak)" do
      other = create(:email_message, email_account: create(:email_account, workspace: create(:workspace)))

      post path_for(other, "archive"), headers: write_headers

      expect(response).to have_http_status(:not_found)
    end
  end
end
