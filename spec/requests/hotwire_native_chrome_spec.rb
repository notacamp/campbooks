require "rails_helper"

RSpec.describe "Hotwire Native chrome", type: :request do
  let(:user) { create(:user) }
  let(:native_ua) { "Campbooks/1.0 Hotwire Native iOS" }
  # The web topbar header has this distinctive class signature (app/views/shared/_topbar).
  let(:topbar_signature) { "sticky top-0 z-40 flex h-14 items-center gap-2 bg-sidebar" }

  before { sign_in(user) }

  it "hides the web topbar and adds the hotwire-native body class for native requests" do
    get root_path, headers: { "HTTP_USER_AGENT" => native_ua }

    expect(response).to have_http_status(:ok)
    expect(response.body).to include('class="h-full hotwire-native"')
    expect(response.body).not_to include(topbar_signature)
  end

  it "renders the topbar and no native class for ordinary web requests" do
    get root_path

    expect(response).to have_http_status(:ok)
    expect(response.body).to include('class="h-full "')
    expect(response.body).to include(topbar_signature)
  end
end
