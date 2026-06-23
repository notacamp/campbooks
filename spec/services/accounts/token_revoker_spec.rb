require "rails_helper"

RSpec.describe Accounts::TokenRevoker do
  let(:workspace) { create(:workspace) }

  def stub_client(account, supports: true)
    client = double("oauth_client")
    allow(client).to receive(:respond_to?).with(:revoke_token).and_return(supports)
    allow(client).to receive(:revoke_token).and_return(true)
    allow(account).to receive(:oauth_client).and_return(client)
    client
  end

  it "revokes when no other active account shares the refresh token" do
    account = create(:email_account, workspace: workspace, refresh_token: "tok-solo", active: false)
    client = stub_client(account)

    expect(client).to receive(:revoke_token)
    described_class.revoke_if_unshared(account)
  end

  it "does NOT revoke when an active sibling shares the same grant (shared-grant hazard)" do
    create(:calendar_account, workspace: workspace, refresh_token: "tok-shared", active: true)
    account = create(:email_account, workspace: workspace, refresh_token: "tok-shared", active: false)
    client = stub_client(account)

    expect(client).not_to receive(:revoke_token)
    described_class.revoke_if_unshared(account)
  end

  it "is best-effort and swallows client errors" do
    account = create(:email_account, workspace: workspace, refresh_token: "tok", active: false)
    client = double("oauth_client")
    allow(client).to receive(:respond_to?).with(:revoke_token).and_return(true)
    allow(client).to receive(:revoke_token).and_raise(StandardError, "network")
    allow(account).to receive(:oauth_client).and_return(client)

    expect { described_class.revoke_if_unshared(account) }.not_to raise_error
  end
end
