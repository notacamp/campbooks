require "rails_helper"

# The personal-data export is now an async job (see account_export_spec.rb for the
# queue/download flow). This file pins the auth guard on the export route.
RSpec.describe "Settings data export", type: :request do
  it "requires authentication" do
    post export_settings_account_path
    expect(response).to redirect_to("/session/new")
  end
end
