require "rails_helper"

# Tests for the provider-not-enabled detection paths in Google::MailClient.
# Uses WebMock to simulate the Gmail API; allocates the client directly so
# the OAuth token-refresh POST is never made.
# Converted from test/services/google/mail_client_test.rb.
RSpec.describe Google::MailClient, "MailboxUnavailable detection" do
  let(:labels_url) { "https://gmail.googleapis.com/gmail/v1/users/me/labels" }

  let(:client) do
    c = described_class.allocate
    # Allocate without calling initialize to skip the OauthClient constructor,
    # then inject a fake oauth object that returns a token immediately.
    fake_oauth = double("oauth", access_token: "fake_access_token")
    c.instance_variable_set(:@oauth, fake_oauth)
    c.instance_variable_set(:@next_page_token, nil)
    c
  end

  before { WebMock.disable_net_connect! }

  context "when the provider returns 400 with 'Mail service not enabled'" do
    before do
      stub_request(:get, labels_url)
        .to_return(
          status: 400,
          body: '{"error":{"code":400,"status":"FAILED_PRECONDITION","message":"Mail service not enabled"}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "raises Emails::MailboxUnavailable" do
      expect { client.list_labels }.to raise_error(Emails::MailboxUnavailable)
    end
  end

  context "when the provider returns a generic 400 (different message)" do
    before do
      stub_request(:get, labels_url)
        .to_return(
          status: 400,
          body: '{"error":{"code":400,"message":"Bad Request"}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "does not raise Emails::MailboxUnavailable and returns an empty array" do
      result = client.list_labels
      expect(result).to eq([])
    end
  end

  context "when the provider returns 401" do
    before do
      stub_request(:get, labels_url)
        .to_return(
          status: 401,
          body: '{"error":{"code":401,"message":"Invalid Credentials"}}',
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "does not raise Emails::MailboxUnavailable (a 401 is a token issue, not a missing mailbox)" do
      # The MailClient returns [] on non-mailbox-unavailable non-200 responses
      # (defensive behaviour in list_labels_raw). A 401 must not trigger the
      # mailbox-deactivation path.
      expect { client.list_labels }.not_to raise_error
    end
  end

  context "when the provider returns a successful response with labels" do
    before do
      stub_request(:get, labels_url)
        .to_return(
          status: 200,
          body: { "labels" => [ { "id" => "INBOX", "name" => "INBOX", "type" => "system" } ] }.to_json,
          headers: { "Content-Type" => "application/json" }
        )
    end

    it "returns the labels array" do
      result = client.list_labels
      expect(result.size).to eq(1)
      expect(result.first["id"]).to eq("INBOX")
    end
  end
end
