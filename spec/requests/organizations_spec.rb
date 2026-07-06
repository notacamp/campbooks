require "rails_helper"
RSpec.describe "Organizations", type: :request do
  let(:ws) { create(:workspace) }
  let(:user) { create(:user, workspace: ws) }
  let(:org) { create(:organization, workspace: ws) }

  before do
    allow_any_instance_of(Entitlements::Resolver).to receive(:allow?).and_return(:ok)
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
    delete session_path
    get organizations_path
    expect(response).to redirect_to(new_session_path)
  end

  it "POST /organizations/backfill runs backfill" do
    expect_any_instance_of(Organizations::Backfill).to receive(:call).and_return(3)
    post backfill_organizations_path
    expect(response).to redirect_to(organizations_path)
  end

  it "blocks when not entitled" do
    allow_any_instance_of(Entitlements::Resolver).to receive(:allow?).with(:organizations).and_return(:not_allowed)
    get organizations_path
    expect(response).to have_http_status(:redirect)
  end

  describe "search" do
    it "filters the directory by name" do
      create(:organization, workspace: ws, name: "Globex Industries")
      create(:organization, workspace: ws, name: "Initech Software")

      get organizations_path(q: "globex")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Globex Industries")
      expect(response.body).not_to include("Initech Software")
    end

    it "also matches the email domain" do
      create(:organization, workspace: ws, name: "Umbrella Group", domain: "umbrella.co")
      create(:organization, workspace: ws, name: "Stark Holdings", domain: "stark.io")

      get organizations_path(q: "umbrella.co")

      expect(response.body).to include("Umbrella Group")
      expect(response.body).not_to include("Stark Holdings")
    end

    it "renders the empty state when nothing matches" do
      create(:organization, workspace: ws, name: "Wayne Enterprises")

      get organizations_path(q: "zzqx-nothing-matches-xyzzy")

      expect(response.body).to include(I18n.t("organizations.index.no_matches"))
      expect(response.body).not_to include("Wayne Enterprises")
    end
  end

  describe "infinite scroll" do
    it "shows the lazy pagination sentinel when there is a next page" do
      create_list(:organization, 31, workspace: ws)

      get organizations_path

      expect(response.body).to include('id="organizations_pagination"')
    end

    it "streams the next page as a turbo stream that appends to the list" do
      create_list(:organization, 31, workspace: ws)

      get organizations_path(page: 2, format: :turbo_stream)

      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include('target="organizations_list"')
    end
  end
end
