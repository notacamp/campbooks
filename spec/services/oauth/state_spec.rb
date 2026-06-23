require "rails_helper"

RSpec.describe Oauth::State do
  it "round-trips a signed payload and marks it verified" do
    token = described_class.encode(flow: "account_link", native: true, user_id: 7)
    data = described_class.decode(token)

    expect(data).to include(
      "flow" => "account_link", "native" => true, "user_id" => 7, "verified" => true
    )
  end

  it "drops nil fields so the web flow doesn't carry an identity" do
    data = described_class.decode(described_class.encode(flow: "sign_in", user_id: nil))
    expect(data).not_to have_key("user_id")
  end

  it "produces a url-safe token (survives the OAuth redirect round-trip)" do
    token = described_class.encode(flow: "sign_in", native: true)
    expect(token).to match(/\A[A-Za-z0-9_\-.]+\z/)
  end

  it "treats a legacy unsigned JSON state as unverified" do
    data = described_class.decode({ "flow" => "drive_link" }.to_json)
    expect(data).to include("flow" => "drive_link", "verified" => false)
  end

  it "rejects an expired token" do
    expired = described_class.encode(flow: "sign_in", native: true, expires_in: -1.second)
    expect(described_class.decode(expired)).to eq({})
  end

  it "returns empty for garbage or blank input" do
    expect(described_class.decode("not-a-real-token")).to eq({})
    expect(described_class.decode(nil)).to eq({})
    expect(described_class.decode("")).to eq({})
  end
end
