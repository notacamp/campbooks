require "rails_helper"

RSpec.describe Microsoft::OauthClient do
  describe "#revoke_token" do
    # Microsoft has no per-refresh-token revoke endpoint for our delegated
    # scopes, so this is a transparent no-network best-effort that returns false.
    # WebMock raises on any unstubbed request, so the absence of an error proves
    # no HTTP call was attempted.
    it "returns false and makes no HTTP call" do
      expect(described_class.new(refresh_token: "rt-123").revoke_token).to be(false)
    end
  end
end
