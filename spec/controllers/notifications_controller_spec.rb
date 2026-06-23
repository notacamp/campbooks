require "rails_helper"

RSpec.describe NotificationsController, type: :controller do
  let(:user) { create(:user) }

  before do
    allow(controller).to receive(:require_authentication).and_return(true)
    allow(controller).to receive(:current_user).and_return(user)
  end

  describe "GET show" do
    it "marks the notification read and redirects to its link" do
      notification = create(:notification, user: user, link_url: "/documents/42", read: false)

      get :show, params: { id: notification.id }

      expect(notification.reload.read).to be(true)
      expect(response).to redirect_to("/documents/42")
    end

    it "falls back to the inbox when the notification has no link" do
      notification = create(:notification, user: user, link_url: nil)

      get :show, params: { id: notification.id }

      expect(response).to redirect_to(notifications_path)
    end

    it "redirects gracefully instead of raising when the notification is gone" do
      expect {
        get :show, params: { id: 999_999 }
      }.not_to raise_error

      expect(response).to redirect_to(notifications_path)
      expect(flash[:info]).to be_present
    end

    it "does not expose or mutate another user's notification" do
      stranger = create(:notification, user: create(:user), read: false)

      get :show, params: { id: stranger.id }

      expect(response).to redirect_to(notifications_path)
      expect(stranger.reload.read).to be(false)
    end
  end

  describe "POST mark_read" do
    it "removes the stale row instead of erroring when already cleared" do
      expect {
        post :mark_read, params: { id: 999_999 }, format: :turbo_stream
      }.not_to raise_error

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("notification_999999")
    end
  end
end
