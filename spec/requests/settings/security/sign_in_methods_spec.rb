require "rails_helper"

RSpec.describe "Settings::Security::SignInMethods", type: :request do
  let(:user) { create(:user) }

  def state_from(location)
    Oauth::State.decode(CGI.parse(URI(location).query)["state"].first)
  end

  describe "POST (start linking a provider)" do
    before { sign_in(user) }

    it "redirects to the provider with an add_sign_in state carrying the user id" do
      allow(Google::OauthClient).to receive(:authorize_url) do |redirect_uri:, state:|
        "https://accounts.google.com/o/oauth2/auth?state=#{state}"
      end

      post settings_security_sign_in_methods_path(provider: "google")

      expect(response.location).to match(%r{accounts\.google\.com})
      expect(state_from(response.location)).to include(
        "flow" => "add_sign_in", "user_id" => user.id, "verified" => true
      )
    end

    it "rejects an unknown provider" do
      post settings_security_sign_in_methods_path(provider: "facebook")
      expect(response).to redirect_to(settings_security_path)
      expect(flash[:error]).to be_present
    end

    it "is a no-op when that provider is already linked" do
      create(:identity, user: user, provider: "google", uid: "g-x")
      post settings_security_sign_in_methods_path(provider: "google")
      expect(response).to redirect_to(settings_security_path)
    end
  end

  describe "DELETE (unlink a provider)" do
    it "removes an identity when the user still has another method" do
      identity = create(:identity, user: user, provider: "google", uid: "g-1")
      sign_in(user)

      expect {
        delete settings_security_sign_in_method_path(identity), params: { current_password: "password123" }
      }.to change { user.identities.count }.by(-1)
      expect(response).to redirect_to(settings_security_path)
    end

    it "requires password re-auth for a password user" do
      identity = create(:identity, user: user, provider: "google", uid: "g-1")
      sign_in(user)

      expect {
        delete settings_security_sign_in_method_path(identity) # no current_password
      }.not_to change(Identity, :count)
      expect(flash[:error]).to be_present
    end

    it "refuses to remove the user's only sign-in method" do
      oauth_only = create(:user, password_set_by_user: false)
      identity = create(:identity, user: oauth_only, provider: "google", uid: "g-9")
      sign_in(oauth_only)

      expect {
        delete settings_security_sign_in_method_path(identity)
      }.not_to change(Identity, :count)
      expect(flash[:error]).to be_present
    end
  end
end
