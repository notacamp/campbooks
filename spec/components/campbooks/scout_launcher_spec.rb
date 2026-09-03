require "rails_helper"

# The compact Scout launcher: the floating spark the email layout uses (in place of
# the docked bar) to open the overlay.
RSpec.describe Campbooks::ScoutLauncher, type: :component do
  it "renders a labelled button that opens the overlay" do
    html = ApplicationController.render(described_class.new, layout: false)
    expect(html).to include("<button")
    expect(html).to include("scout-overlay#open")
    expect(html).to include('aria-label="Ask Scout"')
    # Floats above the bottom nav on mobile, bottom-right on desktop.
    expect(html).to include("fixed")
    expect(html).to include("lg:bottom-4")
  end
end
