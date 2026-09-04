require "rails_helper"

RSpec.describe Campbooks::ScoutBar, type: :component do
  def render_bar(**opts)
    ApplicationController.render(described_class.new(**{ placeholder: "Ask Scout..." }.merge(opts)), layout: false)
  end

  it "renders the desktop bar as a button opening the Scout overlay with the placeholder" do
    html = render_bar
    expect(html).to include("Ask Scout...")
    expect(html).to include("lg:flex")             # desktop-only bar wrapper
    expect(html).to include("<button")
    expect(html).to include("scout-overlay#open")
    expect(html).to include("scout-overlay#openFromKey")
  end

  it "omits the mobile bar unless asked for it" do
    expect(render_bar).not_to include("lg:hidden")
    expect(render_bar(mobile: true, mobile_placeholder: "Ask...")).to include("lg:hidden")
  end

  it "adds the ⌘K keycap only when requested" do
    expect(render_bar).not_to include("⌘K")
    expect(render_bar(keycap: true)).to include("⌘K")
  end

  it "marks the desktop bar as the Scout coach anchor when asked (Home's behaviour)" do
    expect(render_bar(coach_anchor: true)).to include("data-scout-coach-anchor")
  end
end
