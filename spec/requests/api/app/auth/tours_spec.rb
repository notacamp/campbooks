# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app tours", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:headers) { api_app_headers(user) }

  describe "POST /api/app/tours/:key/dismiss" do
    it "marks the tour as dismissed and returns 204" do
      post "/api/app/tours/skim_intro/dismiss", headers: headers

      expect(response).to have_http_status(:no_content)
      expect(user.reload.tour_dismissed?("skim_intro")).to be true
    end

    it "is idempotent — dismissing the same tour twice returns 204 both times" do
      post "/api/app/tours/skim_intro/dismiss", headers: headers
      post "/api/app/tours/skim_intro/dismiss", headers: headers

      expect(response).to have_http_status(:no_content)
      expect(user.reload.dismissed_tours.count("skim_intro")).to eq(1)
    end

    it "accepts unknown tour keys silently" do
      post "/api/app/tours/future_tour_key/dismiss", headers: headers
      expect(response).to have_http_status(:no_content)
    end

    it "requires authentication" do
      post "/api/app/tours/skim_intro/dismiss"
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
