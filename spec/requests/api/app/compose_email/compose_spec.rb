# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app compose", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true, owner: true)
  end

  def headers = api_app_headers(user)

  # ── send ────────────────────────────────────────────────────────────────────

  describe "POST /api/app/compose/send" do
    let(:sender_result) do
      Emails::Sender::Result.success(
        email_message: nil,
        provider_message_id: "msg_123"
      )
    end

    before do
      allow(Emails::Sender).to receive(:call).and_return(sender_result)
    end

    it "returns 201 with the new message id on success" do
      post "/api/app/compose/send",
           params: { to_address: "to@example.com", subject: "Hello", body: "<p>hi</p>",
                     email_account_id: account.id },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(Emails::Sender).to have_received(:call).with(hash_including(user: user, to_address: "to@example.com"))
    end

    it "returns 422 when Sender fails" do
      allow(Emails::Sender).to receive(:call)
        .and_return(Emails::Sender::Result.failure("send_failed", "Provider rejected the message."))

      post "/api/app/compose/send",
           params: { to_address: "to@example.com", body: "hi" },
           headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "401s without a token" do
      post "/api/app/compose/send", params: { to_address: "x@x.com", body: "hi" }
      expect(response).to have_http_status(:unauthorized)
    end
  end

  # ── reply ────────────────────────────────────────────────────────────────────

  describe "POST /api/app/compose/reply" do
    let!(:thread) { create(:email_thread, email_account: account) }
    let!(:source)  do
      create(:email_message, email_account: account, email_scan_log: scan_log,
                             email_thread: thread, from_address: "sender@example.com")
    end
    let(:sender_result) { Emails::Sender::Result.success(email_message: nil, provider_message_id: "r_1") }

    before do
      allow(Emails::Sender).to receive(:call).and_return(sender_result)
    end

    it "returns 201 for a valid reply" do
      post "/api/app/compose/reply",
           params: { email_message_id: source.id, body: "<p>Reply body</p>" },
           headers: headers

      expect(response).to have_http_status(:created)
      expect(Emails::Sender).to have_received(:call).with(
        hash_including(source_message: source, user: user)
      )
    end

    it "404s when replying to an inaccessible message" do
      other_workspace = create(:workspace)
      other_account   = create(:email_account, workspace: other_workspace)
      other_scan_log  = create(:email_scan_log, email_account: other_account)
      other_message   = create(:email_message, email_account: other_account, email_scan_log: other_scan_log)

      post "/api/app/compose/reply",
           params: { email_message_id: other_message.id, body: "hi" },
           headers: headers

      expect(response).to have_http_status(:not_found)
    end
  end

  # ── rewrite ──────────────────────────────────────────────────────────────────

  describe "POST /api/app/compose/rewrite" do
    before do
      allow_any_instance_of(Ai::DraftRewriter).to receive(:rewrite).and_return("<p>Shorter.</p>")
    end

    it "returns 200 with rewritten body_html" do
      post "/api/app/compose/rewrite",
           params: { body_html: "<p>Original long text.</p>", tone: "shorter" },
           headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["body_html"]).to eq("<p>Shorter.</p>")
      expect(data["tone"]).to eq("shorter")
    end

    it "returns 422 when rewriter returns nil" do
      allow_any_instance_of(Ai::DraftRewriter).to receive(:rewrite).and_return(nil)

      post "/api/app/compose/rewrite",
           params: { body_html: "<p>text</p>", tone: "warmer" },
           headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  # ── prefill ──────────────────────────────────────────────────────────────────

  describe "GET /api/app/compose/prefill" do
    it "returns 200 with prefill data" do
      get "/api/app/compose/prefill", params: { to: "contact@example.com" }, headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data).to include("to", "subject", "to_inferred", "subject_inferred")
    end
  end

  # ── chat ─────────────────────────────────────────────────────────────────────

  describe "POST /api/app/compose/chat" do
    before do
      allow(ComposeChatReplyJob).to receive(:perform_later)
    end

    it "returns 201 with message id and enqueues the reply job" do
      post "/api/app/compose/chat",
           params: { content: "Draft a reply about the Q3 budget" },
           headers: headers

      expect(response).to have_http_status(:created)
      data = response.parsed_body["data"]
      expect(data["id"]).to be_present
      expect(data["thread_id"]).to be_present
      expect(data["status"]).to eq("processing")
      expect(ComposeChatReplyJob).to have_received(:perform_later)
    end
  end
end
