# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Members API", type: :request do
  let(:workspace) { create(:workspace) }
  let(:admin) do
    create(:user, workspace: workspace, role: :admin,
                  password: "secret1234", password_confirmation: "secret1234")
  end
  let(:member) do
    create(:user, workspace: workspace, role: :member,
                  password: "secret1234", password_confirmation: "secret1234")
  end

  describe "GET /api/app/settings/members" do
    it "returns members list" do
      admin
      member
      get "/api/app/settings/members", headers: api_app_headers(admin)

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data["members"].size).to eq(2)
      expect(data).to have_key("invitations")
    end

    it "also works for non-admin members (read)" do
      get "/api/app/settings/members", headers: api_app_headers(member)
      expect(response).to have_http_status(:ok)
    end

    it "returns 401 without auth" do
      get "/api/app/settings/members"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "PATCH /api/app/settings/members/:id" do
    it "allows admin to change a member role" do
      patch "/api/app/settings/members/#{member.id}",
            params: { role: "admin" },
            headers: api_app_headers(admin)

      expect(response).to have_http_status(:ok)
      expect(member.reload.role).to eq("admin")
    end

    it "returns 403 when a non-admin member tries to update roles" do
      another_member = create(:user, workspace: workspace, role: :member,
                                     password: "secret1234", password_confirmation: "secret1234")

      patch "/api/app/settings/members/#{another_member.id}",
            params: { role: "admin" },
            headers: api_app_headers(member)

      expect(response).to have_http_status(:forbidden)
      expect(another_member.reload.role).to eq("member")
    end

    it "returns 404 for a member in a different workspace" do
      other_workspace = create(:workspace)
      outsider = create(:user, workspace: other_workspace, role: :member,
                               password: "secret1234", password_confirmation: "secret1234")

      patch "/api/app/settings/members/#{outsider.id}",
            params: { role: "admin" },
            headers: api_app_headers(admin)

      expect(response).to have_http_status(:not_found)
    end
  end
end
