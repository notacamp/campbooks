require "rails_helper"

# Regression guard for the native double-browser bug: in the native app the
# OAuth buttons must link straight to the provider authorize URL. Going via the
# /session/:provider redirect made Hotwire Native both URLSession-preflight and
# WebView-navigate the link, opening the system browser twice.
RSpec.describe "Native OAuth login links", type: :request do
  let(:native_ua) { "Campbooks/1.0 Hotwire Native iOS" }

  it "links OAuth buttons straight to the provider authorize URL in the native app" do
    get new_session_path, headers: { "HTTP_USER_AGENT" => native_ua }

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("https://login.microsoftonline.com")
    expect(response.body).not_to match(%r{href="/session/microsoft"})
  end

  it "uses the /session/:provider redirect for ordinary web requests" do
    get new_session_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to match(%r{href="/session/microsoft"})
    expect(response.body).not_to include("https://login.microsoftonline.com")
  end
end
