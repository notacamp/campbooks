require "rails_helper"

RSpec.describe Zoho::OauthClient do
  describe "#revoke_token" do
    subject(:client) { described_class.new(refresh_token: "rt-123") }

    let(:conn) { instance_double(Faraday::Connection) }

    before { allow(client).to receive(:connection).and_return(conn) }

    def response(status, body = "")
      instance_double(Faraday::Response, success?: (200..299).cover?(status), status: status, body: body)
    end

    it "POSTs to Zoho's EU revoke endpoint and returns true on success" do
      expect(conn).to receive(:post).with("https://accounts.zoho.eu/oauth/v2/token/revoke").and_return(response(200, '{"status":"success"}'))
      expect(client.revoke_token).to be(true)
    end

    it "returns false on a non-2xx response without raising" do
      allow(conn).to receive(:post).and_return(response(401, '{"error":"invalid"}'))
      expect(client.revoke_token).to be(false)
    end

    it "returns false without a refresh token (no HTTP call)" do
      expect(described_class.new(refresh_token: nil).revoke_token).to be(false)
    end
  end
end
