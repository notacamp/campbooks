require "rails_helper"

# The Scout overlay + docked bar is wired into both layouts. Classic is gone;
# every user gets the overlay.
RSpec.describe "Scout overlay layout wiring", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  it "renders the Scout overlay + docked bar on the application layout (not the palette)" do
    get now_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("scout-overlay-dialog")
    expect(response.body).to include("scout-overlay#open")           # the docked bar button
    expect(response.body).to match(/data-controller="scout-overlay /) # body controller
  end

  it "renders the floating launcher on the email layout, but not on /scout itself" do
    get scout_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("scout-overlay-dialog")
    # The Scout page suppresses its own launcher (layout_scout_launcher?).
    expect(response.body).not_to include('aria-label="Ask Scout"')
  end
end
