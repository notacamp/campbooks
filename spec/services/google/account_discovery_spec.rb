# frozen_string_literal: true

require "rails_helper"

RSpec.describe Google::AccountDiscovery do
  def stub_userinfo(body)
    stub_request(:get, "https://www.googleapis.com/oauth2/v2/userinfo")
      .to_return(status: 200, body: body.to_json, headers: { "Content-Type" => "application/json" })
  end

  it "surfaces email_verified: true when Google reports verified_email" do
    stub_userinfo(email: "a@example.com", name: "A", id: "42", verified_email: true)

    expect(described_class.new("tok").discover_identity)
      .to include(email: "a@example.com", account_id: "42", email_verified: true)
  end

  it "surfaces email_verified: false when Google reports the email is unverified" do
    stub_userinfo(email: "a@example.com", id: "42", verified_email: false)

    expect(described_class.new("tok").discover_identity[:email_verified]).to be(false)
  end

  it "treats a missing verified flag as unverified (safe default)" do
    stub_userinfo(email: "a@example.com", id: "42")

    expect(described_class.new("tok").discover_identity[:email_verified]).to be(false)
  end

  it "also accepts the OIDC-style email_verified key" do
    stub_userinfo(email: "a@example.com", id: "42", email_verified: true)

    expect(described_class.new("tok").discover_identity[:email_verified]).to be(true)
  end

  it "returns nil when there is no email" do
    stub_userinfo(id: "42")

    expect(described_class.new("tok").discover_identity).to be_nil
  end
end
