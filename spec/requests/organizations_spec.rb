require "rails_helper"
RSpec.describe "Organizations", type: :request do
  let(:ws) { create(:workspace) }
  let(:user) { create(:user, workspace: ws) }
  let(:org) { create(:organization, workspace: ws) }

  before do
    allow_any_instance_of(Entitlements::Resolver).to receive(:feature?).and_return(false)
    allow_any_instance_of(Entitlements::Resolver).to receive(:feature?).with(:organizations).and_return(true)
    sign_in(user)
  end

  it "GET /organizations returns 200" do
    get organizations_path
    expect(response).to have_http_status(:ok)
  end

  it "GET /organizations/:id returns 200" do
    get organization_path(org)
    expect(response).to have_http_status(:ok)
  end

  it "requires authentication" do
    delete sign_out_path rescue nil
    get organizations_path
    expect(response).to redirect_to(new_session_path)
  end

  it "POST /organizations/backfill runs backfill" do
    expect_any_instance_of(Organizations::Backfill).to receive(:call).and_return(3)
    post backfill_organizations_path
    expect(response).to redirect_to(organizations_path)
  end

  it "blocks when not entitled" do
    allow_any_instance_of(Entitlements::Resolver).to receive(:feature?).with(:organizations).and_return(false)
    get organizations_path
    expect(response).to have_http_status(:redirect)
  end
end
