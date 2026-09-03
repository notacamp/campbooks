require "rails_helper"

RSpec.describe Campbooks::ScoutBar, type: :component do
  include Rails.application.routes.url_helpers
  def render_bar(**opts)
    ApplicationController.render(described_class.new(**{ placeholder: "Ask Scout…" }.merge(opts)), layout: false)
  end

  it "renders the desktop bar linking to Scout by default, with the placeholder" do
    html = render_bar
    expect(html).to include(scout_path)
    expect(html).to include("Ask Scout…")
    expect(html).to include("lg:flex")           # desktop-only bar
  end

  it "omits the mobile bar unless asked for it" do
    expect(render_bar).not_to include("lg:hidden")
    expect(render_bar(mobile: true, mobile_placeholder: "Ask…")).to include("lg:hidden")
  end

  it "adds the ⌘K keycap only when requested" do
    expect(render_bar).not_to include("⌘K")
    expect(render_bar(keycap: true)).to include("⌘K")
  end

  it "marks the desktop bar as the Scout coach anchor when asked (Home's behaviour)" do
    expect(render_bar(coach_anchor: true)).to include("data-scout-coach-anchor")
  end

  it "honours a custom href and desktop max width" do
    html = render_bar(href: "/somewhere", desktop_max_width: "max-w-[680px]")
    expect(html).to include('href="/somewhere"')
    expect(html).to include("max-w-[680px]")
  end
end
