require "rails_helper"

RSpec.describe Microsoft::OauthClient do
  # The client ENV.fetches its app credentials at construction; supply dummies
  # for the duration of each example (CI has no real keys).
  around { |example| with_env("MICROSOFT_CLIENT_ID" => "test-id", "MICROSOFT_CLIENT_SECRET" => "test-secret") { example.run } }

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
