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
end
