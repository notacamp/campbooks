require "rails_helper"

# Tests for Google::MailClient#watch and #stop_watch.
# Uses WebMock to simulate the Gmail API; allocates the client directly so
# the OAuth token-refresh POST is never made (mirrors mail_client_spec.rb).
RSpec.describe Google::MailClient, "watch methods" do
  let(:watch_url) { "https://gmail.googleapis.com/gmail/v1/users/me/watch" }
  let(:stop_url)  { "https://gmail.googleapis.com/gmail/v1/users/me/stop"  }

  let(:client) do
    c = described_class.allocate
    fake_oauth = double("oauth", access_token: "fake_access_token")
    c.instance_variable_set(:@oauth, fake_oauth)
    c.instance_variable_set(:@next_page_token, nil)
    fake_account = double("email_account", refresh_token: "tok", workspace_id: nil, try: nil)
    c.instance_variable_set(:@email_account, fake_account)
    c
  end

  before { WebMock.disable_net_connect! }

  describe "#watch" do
    context "with a successful response" do
      # Gmail returns `expiration` as epoch-milliseconds in a STRING.
      let(:expiration_ms) { (Time.now.utc + 7.days).to_i * 1000 }

      before do
        stub_request(:post, watch_url)
          .to_return(
            status: 200,
            body: { "historyId" => "99", "expiration" => expiration_ms.to_s }.to_json,
            headers: { "Content-Type" => "application/json" }
          )
      end

      it "returns a hash with :history_id and :expires_at" do
        result = client.watch("projects/p/topics/t")
        expect(result[:history_id]).to eq("99")
      end

      it "parses the expiration millisecond string into a Time" do
        result = client.watch("projects/p/topics/t")
        expected = Time.at(expiration_ms / 1000.0)
        expect(result[:expires_at]).to be_within(1.second).of(expected)
      end

      it "POSTs the topic name as topicName in the request body" do
        client.watch("projects/p/topics/my-topic")
        expect(WebMock).to have_requested(:post, watch_url)
          .with(body: { "topicName" => "projects/p/topics/my-topic" })
      end
    end

    context "when the mailbox has Gmail disabled (400 FAILED_PRECONDITION)" do
      before do
        stub_request(:post, watch_url)
          .to_return(
            status: 400,
            body: '{"error":{"code":400,"status":"FAILED_PRECONDITION","message":"Mail service not enabled"}}',
            headers: { "Content-Type" => "application/json" }
          )
      end

      it "raises Emails::MailboxUnavailable" do
        expect { client.watch("projects/p/topics/t") }
          .to raise_error(Emails::MailboxUnavailable)
      end
    end
  end

  describe "#stop_watch" do
    context "with a successful response" do
      before do
        stub_request(:post, stop_url).to_return(status: 200, body: "")
      end

      it "returns true" do
        expect(client.stop_watch).to be(true)
      end
    end

    context "when the request raises a network error" do
      before do
        stub_request(:post, stop_url).to_raise(Faraday::ConnectionFailed.new("timeout"))
      end

      it "returns false and does not raise" do
        expect { expect(client.stop_watch).to be(false) }.not_to raise_error
      end
    end

    context "when the server returns a non-2xx" do
      before do
        stub_request(:post, stop_url).to_return(status: 404, body: "Not Found")
      end

      it "returns true (non-2xx from stop is still a Faraday success — not a raise)" do
        # Faraday does not raise on 4xx; the method posts and returns true.
        expect(client.stop_watch).to be(true)
      end
    end
  end
end
