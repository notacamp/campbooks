require "rails_helper"

RSpec.describe "Organizations", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:org) { create(:organization, workspace: workspace) }

  def enable_feature!
    allow(Features).to receive(:organizations?).and_return(true)
  end

  describe "when feature flag is off" do
    before { sign_in(user) }

    it "GET /organizations returns 404" do
      get organizations_path
      expect(response).to have_http_status(:not_found)
    end

    it "GET /organizations/:id returns 404" do
      get organization_path(org)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "when feature flag is on" do
    before { enable_feature! }

    describe "GET /organizations" do
      it "returns the index page" do
        sign_in(user)
        get organizations_path
        expect(response).to have_http_status(:ok)
      end

      it "requires authentication" do
        get organizations_path
        expect(response).to redirect_to(new_session_path)
      end
    end

    describe "GET /organizations/:id" do
      it "returns the show page for an org in the user's workspace" do
        sign_in(user)
        get organization_path(org)
        expect(response).to have_http_status(:ok)
      end

      it "returns 404 for an org in another workspace" do
        other_org = create(:organization)
        sign_in(user)
        get organization_path(other_org)
        expect(response).to have_http_status(:not_found)
      end

      it "requires authentication" do
        get organization_path(org)
        expect(response).to redirect_to(new_session_path)
      end
    end

    describe "PATCH /organizations/:id" do
      it "updates the organization name" do
        sign_in(user)
        patch organization_path(org), params: { organization: { name: "New Name" } }
        expect(response).to redirect_to(organization_path(org))
        expect(org.reload.name).to eq("New Name")
      end
    end

    describe "GET /organizations/:id/emails" do
      it "renders the emails partial" do
        sign_in(user)
        get emails_organization_path(org)
        expect(response).to have_http_status(:ok)
      end
    end

    describe "GET /organizations/:id/documents" do
      it "renders the documents partial" do
        sign_in(user)
        get documents_organization_path(org)
        expect(response).to have_http_status(:ok)
      end
    end

    describe "POST /organizations/backfill" do
      it "runs the backfill and redirects" do
        sign_in(user)
        expect_any_instance_of(Organizations::Backfill).to receive(:call).and_return(3)
        post backfill_organizations_path
        expect(response).to redirect_to(organizations_path)
      end
    end
  end
end
