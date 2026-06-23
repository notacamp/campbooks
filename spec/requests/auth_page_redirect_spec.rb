require "rails_helper"

# Auth forms (sign in, forgot password) render under the default `application`
# layout. Without a guard, an already-signed-in visitor lands on the form wrapped
# in the full authenticated app chrome (nav rail, topbar, drawers). These pages
# should bounce a signed-in user back into the app instead.
RSpec.describe "Auth pages for an already signed-in visitor", type: :request do
  let(:user) { create(:user) }

  context "when already signed in" do
    before { sign_in(user) }

    it "redirects GET /session/new into the app" do
      get new_session_path
      expect(response).to redirect_to(root_path)
    end

    it "redirects GET /passwords/new (forgot password) into the app" do
      get new_password_path
      expect(response).to redirect_to(root_path)
    end
  end

  context "when not signed in" do
    it "still renders the sign-in form" do
      get new_session_path
      expect(response).to have_http_status(:ok)
    end

    it "still renders the forgot-password form" do
      get new_password_path
      expect(response).to have_http_status(:ok)
    end
  end
end
