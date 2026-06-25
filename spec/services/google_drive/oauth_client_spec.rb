require "rails_helper"

RSpec.describe GoogleDrive::OauthClient do
  describe "#revoke_token" do
    around do |example|
      original = ENV.values_at("GOOGLE_DRIVE_CLIENT_ID", "GOOGLE_DRIVE_CLIENT_SECRET")
      ENV["GOOGLE_DRIVE_CLIENT_ID"] = "drive-id"
      ENV["GOOGLE_DRIVE_CLIENT_SECRET"] = "drive-secret"
      example.run
    ensure
      ENV["GOOGLE_DRIVE_CLIENT_ID"], ENV["GOOGLE_DRIVE_CLIENT_SECRET"] = original
    end

    let(:client) { described_class.new }
    let(:connection) { instance_double(Faraday::Connection) }

    before { allow(client).to receive(:connection).and_return(connection) }

    it "posts the token to Google's revoke endpoint and returns true" do
      expect(connection).to receive(:post).with(described_class::REVOKE_URL).and_return(double(status: 200))
      expect(client.revoke_token("rt-123")).to be(true)
    end

    it "treats an already-invalid token (HTTP 400) as revoked" do
      allow(connection).to receive(:post).and_raise(Faraday::BadRequestError.new("400"))
      expect(client.revoke_token("stale")).to be(true)
    end

    it "returns false on a server error" do
      allow(connection).to receive(:post).and_raise(Faraday::ServerError.new("500"))
      expect(client.revoke_token("rt-123")).to be(false)
    end

    it "returns false and makes no HTTP call for a blank token" do
      expect(connection).not_to receive(:post)
      expect(client.revoke_token(nil)).to be(false)
    end
  end
end
