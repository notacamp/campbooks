require "rails_helper"

RSpec.describe "Hotwire Native path configuration", type: :request do
  it "serves the iOS path configuration JSON without authentication" do
    get "/configurations/ios_v1.json"

    expect(response).to have_http_status(:ok)
    expect(response.media_type).to eq("application/json")

    body = JSON.parse(response.body)
    expect(body["rules"]).to be_an(Array)
    expect(body["rules"]).not_to be_empty
  end

  it "serves the Android variant too" do
    get "/configurations/android_v1.json"

    expect(response).to have_http_status(:ok)
    expect(JSON.parse(response.body)).to have_key("rules")
  end

  it "falls back to the default configuration for an unknown platform slug" do
    get "/configurations/bogus_v9.json"

    expect(response).to have_http_status(:ok)
    expect(JSON.parse(response.body)).to have_key("rules")
  end
end
