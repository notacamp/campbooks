require "rails_helper"

RSpec.describe Google::OauthClient do
  # The client ENV.fetches its app credentials at construction; supply dummies
  # for the duration of each example (CI has no real keys).
  around { |example| with_env("GOOGLE_CLIENT_ID" => "test-id", "GOOGLE_CLIENT_SECRET" => "test-secret") { example.run } }

  describe "#revoke_token" do
    subject(:client) { described_class.new(refresh_token: "rt-123") }

    let(:conn) { instance_double(Faraday::Connection) }

    before { allow(client).to receive(:connection).and_return(conn) }

    def response(status, body = "")
      instance_double(Faraday::Response, success?: (200..299).cover?(status), status: status, body: body)
    end

    it "POSTs to Google's revoke endpoint and returns true on 200" do
      expect(conn).to receive(:post).with("https://oauth2.googleapis.com/revoke").and_return(response(200))
      expect(client.revoke_token).to be(true)
    end

    it "treats an already-invalid token (400) as revoked" do
      allow(conn).to receive(:post).and_return(response(400, '{"error":"invalid_token"}'))
      expect(client.revoke_token).to be(true)
    end

    it "returns false on other errors without raising" do
      allow(conn).to receive(:post).and_return(response(500, "boom"))
      expect(client.revoke_token).to be(false)
    end

    it "returns false without a refresh token (no HTTP call)" do
      expect(described_class.new(refresh_token: nil).revoke_token).to be(false)
    end
  end
end
