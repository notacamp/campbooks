require "rails_helper"

# The Scout overlay shell: a <dialog> with the one query input in the head, a lazy
# turbo-frame body, and a conversation foot. Behaviour is driven by
# scout_overlay_controller.js + verified in the browser; here we pin the shell's
# structure and the data hooks the controller depends on.
RSpec.describe Campbooks::ScoutOverlay, type: :component do
  def render_component(**opts)
    ApplicationController.render(described_class.new(**opts), layout: false)
  end

  it "renders the dialog shell: one input, a lazy body frame, and the foot" do
    html = render_component
    expect(html).to include("scout-overlay-dialog")
    expect(html).to include('data-scout-overlay-target="dialog"')
    # A single query input carrying both placeholders (head + moved-to-foot).
    expect(html.scan('data-scout-overlay-target="input"').size).to eq(1)
    expect(html).to include("Ask Scout anything, or type a command")
    expect(html).to include("data-followup-placeholder")
    # The body is a bare turbo-frame (loaded lazily on first open).
    expect(html).to include('id="scout_overlay_body"')
    expect(html).not_to include("src=")
    # Foot: Recent toggle + Open Scout link to the classic page.
    expect(html).to include("scout-overlay#toggleRecent")
    expect(html).to include("Open Scout")
    expect(html).to include(Rails.application.routes.url_helpers.scout_path)
    # Esc keycap + the ask-label the controller reads for the Ask Scout row.
    expect(html).to include("Esc")
    expect(html).to include("data-ask-label")
  end

  it "seeds a browse preview with suggestions and Recent for Lookbook" do
    html = render_component(preview: :browse)
    expect(html).to include("open") # dialog open in preview
    expect(html).to include("Recent")
  end

  it "seeds a conversation preview with Scout speaking" do
    html = render_component(preview: :conversation)
    expect(html).to include("Scout")
  end
end
