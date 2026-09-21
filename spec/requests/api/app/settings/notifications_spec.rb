# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Notifications API", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, password: "secret1234", password_confirmation: "secret1234") }
  let(:other_user) { create(:user, workspace: create(:workspace), password: "secret1234", password_confirmation: "secret1234") }
  let(:headers) { api_app_headers(user) }

  describe "GET /api/app/notifications" do
    let!(:notification) { create(:notification, user: user) }
    let!(:other_notification) { create(:notification, user: other_user) }

    it "returns the current user's notifications" do
      get "/api/app/notifications", headers: headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      ids = data.map { |n| n["id"] }
      expect(ids).to include(notification.id)
      expect(ids).not_to include(other_notification.id)
    end

    it "includes pagination meta" do
      get "/api/app/notifications", headers: headers
      expect(response.parsed_body).to have_key("meta")
    end

    it "401s without auth" do
      get "/api/app/notifications"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "POST /api/app/notifications/:id/mark_read" do
    let!(:notification) { create(:notification, user: user, read: false) }

    it "marks the notification as read" do
      post "/api/app/notifications/#{notification.id}/mark_read", headers: headers

      expect(response).to have_http_status(:ok)
      expect(notification.reload.read).to be true
    end

    it "returns 404 for a notification belonging to another user" do
      other_n = create(:notification, user: other_user)
      post "/api/app/notifications/#{other_n.id}/mark_read", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /api/app/notifications/mark_all_read" do
    let!(:n1) { create(:notification, user: user, read: false) }
    let!(:n2) { create(:notification, user: user, read: false) }

    it "marks all active unread notifications as read" do
      post "/api/app/notifications/mark_all_read", headers: headers

      expect(response).to have_http_status(:ok)
      expect(n1.reload.read).to be true
      expect(n2.reload.read).to be true
    end
  end

  describe "DELETE /api/app/notifications/:id" do
    let!(:notification) { create(:notification, user: user) }

    it "deletes the notification" do
      delete "/api/app/notifications/#{notification.id}", headers: headers

      expect(response).to have_http_status(:no_content)
      expect { notification.reload }.to raise_error(ActiveRecord::RecordNotFound)
    end
  end
end
