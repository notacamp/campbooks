require "rails_helper"

RSpec.describe "Registration", type: :request do
  it "renders the signup form with a Terms + Privacy consent checkbox and links" do
    get new_registration_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include('name="terms_accepted"') # the consent checkbox
    expect(response.body).to include("Terms of Service")
    expect(response.body).to include(ApplicationController.helpers.marketing_url("/terms"))
    expect(response.body).to include(ApplicationController.helpers.marketing_url("/privacy"))
  end

  # ── Native client behavior (RegistrationsControllerTest) ──────────────────

  context "with native vs web signup mode" do
    # turbo-rails identifies the native shell by "(Turbo|Hotwire) Native" in the UA.
    NATIVE_UA = "Campbooks/1.0 iOS Hotwire Native".freeze
    WEB_SIGNUP_LINK = "https://campbooks.not-a-camp.com/".freeze

    around do |example|
      previous_signup_mode = Rails.application.config.signup_mode
      Rails.application.config.signup_mode = :open
      example.run
    ensure
      Rails.application.config.signup_mode = previous_signup_mode
    end

    it "web sign-in page links to in-app account creation" do
      get new_session_path

      expect(response).to have_http_status(:ok)
      doc = Nokogiri::HTML(response.body)
      expect(doc.css("a[href='#{new_registration_path}']")).not_to be_empty
    end

    it "native sign-in page hides in-app signup and points to the web instead" do
      get new_session_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

      expect(response).to have_http_status(:ok)
      doc = Nokogiri::HTML(response.body)
      expect(doc.css("a[href='#{new_registration_path}']")).to be_empty
      expect(doc.css("a[href='#{WEB_SIGNUP_LINK}']")).not_to be_empty
    end

    it "native registration is blocked and redirects to sign-in" do
      get new_registration_path, headers: { "HTTP_USER_AGENT" => NATIVE_UA }

      expect(response).to redirect_to(new_session_path)
      expect(flash[:alert]).to eq(I18n.t("registrations.native_signup_unavailable"))
    end

    it "web registration renders the signup form" do
      get new_registration_path

      expect(response).to have_http_status(:ok)
    end
  end
end
