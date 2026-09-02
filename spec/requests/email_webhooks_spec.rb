require "rails_helper"

RSpec.describe "EmailWebhooks", type: :request do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }

  # Build a valid Pub/Sub push body for the given email address.
  def pubsub_body(email_address: account.email_address, history_id: "12345")
    inner = { "emailAddress" => email_address, "historyId" => history_id }.to_json
    {
      "message" => {
        "data"        => Base64.strict_encode64(inner),
        "messageId"   => "msg-1",
        "publishTime" => "2026-01-01T00:00:00Z"
      },
      "subscription" => "projects/my-project/subscriptions/gmail-sub"
    }.to_json
  end

  # Helpers that stub the GmailPush module into a configured / unconfigured state.
  def stub_push_configured
    allow(Emails::GmailPush).to receive(:configured?).and_return(true)
    allow(Emails::GmailPush).to receive(:token).and_return("secret-token")
    allow(Emails::GmailPush).to receive(:valid_token?).with("secret-token").and_return(true)
    allow(Emails::GmailPush).to receive(:valid_token?).with(anything).and_call_original
    allow(Emails::GmailPush).to receive(:valid_token?).with("secret-token").and_return(true)
  end

  def post_gmail(token: "secret-token", body: pubsub_body)
    post email_webhooks_gmail_path(token: token),
         params: body,
         headers: { "Content-Type" => "application/json" }
  end

  before { ActiveJob::Base.queue_adapter.enqueued_jobs.clear }

  context "when push is not configured" do
    before { allow(Emails::GmailPush).to receive(:configured?).and_return(false) }

    it "returns 404" do
      post_gmail
      expect(response).to have_http_status(:not_found)
    end

    it "does not enqueue a scan job" do
      post_gmail
      expect(ActiveJob::Base.queue_adapter.enqueued_jobs).to be_empty
    end
  end

  context "when push is configured" do
    before do
      allow(Emails::GmailPush).to receive(:configured?).and_return(true)
      allow(Emails::GmailPush).to receive(:valid_token?) do |candidate|
        candidate == "secret-token"
      end
    end

    context "with a bad token" do
      it "returns 404" do
        post_gmail(token: "wrong-token")
        expect(response).to have_http_status(:not_found)
      end
    end

    context "with a valid token" do
      context "and a well-formed envelope for a known account" do
        it "enqueues EmailScanJob with (account_id, 'delta') and returns 204" do
          expect {
            post_gmail
          }.to have_enqueued_job(EmailScanJob).with(account.id, "delta")

          expect(response).to have_http_status(:no_content)
        end

        it "debounces: a second push within the window does not enqueue again" do
          # Use a real in-memory cache so the unless_exist: true write is honoured.
          allow(Rails).to receive(:cache).and_return(ActiveSupport::Cache::MemoryStore.new)

          post_gmail
          ActiveJob::Base.queue_adapter.enqueued_jobs.clear

          expect {
            post_gmail
          }.not_to have_enqueued_job(EmailScanJob)

          expect(response).to have_http_status(:no_content)
        end
      end

      context "with an unknown email address" do
        it "returns 204 and does not enqueue a scan" do
          expect {
            post_gmail(body: pubsub_body(email_address: "nobody@example.com"))
          }.not_to have_enqueued_job(EmailScanJob)

          expect(response).to have_http_status(:no_content)
        end
      end

      context "with a malformed inner payload (invalid Base64 JSON content)" do
        it "returns 204 (ACK) and does not enqueue" do
          # Valid Pub/Sub envelope, but the decoded inner JSON is garbage — a
          # poison message that must be ACKed rather than retried indefinitely.
          bad_body = {
            "message" => {
              "data"      => Base64.strict_encode64("not-valid-json!"),
              "messageId" => "poison-1"
            }
          }.to_json

          expect {
            post email_webhooks_gmail_path(token: "secret-token"),
                 params: bad_body,
                 headers: { "Content-Type" => "application/json" }
          }.not_to have_enqueued_job(EmailScanJob)

          expect(response).to have_http_status(:no_content)
        end
      end

      context "with a body missing the data field" do
        it "returns 204 (ACK) and does not enqueue" do
          bad_body = { "message" => { "messageId" => "1" } }.to_json
          expect {
            post email_webhooks_gmail_path(token: "secret-token"),
                 params: bad_body,
                 headers: { "Content-Type" => "application/json" }
          }.not_to have_enqueued_job(EmailScanJob)

          expect(response).to have_http_status(:no_content)
        end
      end
    end
  end
end
