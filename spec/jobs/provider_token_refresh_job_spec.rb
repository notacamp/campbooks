require "rails_helper"

RSpec.describe ProviderTokenRefreshJob, type: :job do
  let(:workspace) { create(:workspace) }
  let!(:account) { create(:email_account, workspace: workspace, active: true) }

  def stub_refresh(&blk)
    client = instance_double(Zoho::OauthClient)
    allow_any_instance_of(EmailAccount).to receive(:oauth_client).and_return(client)
    allow(client).to receive(:refresh!, &blk)
  end

  it "deactivates the account on a permanent (dead grant) failure" do
    stub_refresh { raise PermanentAuthError, "Zoho token refresh failed: invalid_code" }

    described_class.perform_now

    expect(account.reload.active).to be(false)
  end

  it "leaves the account active on a transient/config failure" do
    # The bug this guards: a server-side blip (bad/missing client creds during a
    # deploy, provider 5xx) raised a generic AuthenticationError and disconnected
    # every account at once. It must now stay active and simply retry next run.
    stub_refresh { raise AuthenticationError, "Zoho token refresh failed: invalid_client" }

    described_class.perform_now

    expect(account.reload.active).to be(true)
  end

  it "keeps the account active when the refresh succeeds" do
    stub_refresh { "fresh-access-token" }

    described_class.perform_now

    expect(account.reload.active).to be(true)
  end
end
