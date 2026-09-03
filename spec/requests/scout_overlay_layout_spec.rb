require "rails_helper"

# The bold layout swaps the classic command palette for the Scout overlay + docked
# bar (application layout) / floating launcher (email layout). Classic is untouched.
RSpec.describe "Scout overlay layout wiring", type: :request do
  let(:workspace) { create(:workspace) }

  before { allow(Features).to receive(:bold_layout?).and_return(true) }

  context "a bold-mode user" do
    let(:user) { create(:user, workspace: workspace, layout_mode: :bold) }
    before { sign_in(user) }

    it "renders the Scout overlay + docked bar on the application layout (not the palette)" do
      get now_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("scout-overlay-dialog")
      expect(response.body).to include("scout-overlay#open")           # the docked bar button
      expect(response.body).not_to include("command-palette-dialog")
      expect(response.body).to match(/data-controller="scout-overlay /) # body controller swap
    end

    it "renders the floating launcher on the email layout, but not on /scout itself" do
      get scout_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("scout-overlay-dialog")
      # The Scout page suppresses its own launcher (layout_scout_launcher?).
      expect(response.body).not_to include('aria-label="Ask Scout"')
    end
  end

  context "a classic-mode user (flag on)" do
    let(:user) { create(:user, workspace: workspace, layout_mode: :classic) }
    before { sign_in(user) }

    it "still gets the classic command palette, not the overlay or bar" do
      get now_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("command-palette-dialog")
      expect(response.body).not_to include("scout-overlay-dialog")
      # The rail/topbar search buttons carry scout-overlay#open in both layouts
      # (only one controller is ever mounted), so assert on the bar-only action.
      expect(response.body).not_to include("scout-overlay#openFromKey")
    end
  end
end
