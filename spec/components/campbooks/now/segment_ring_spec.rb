require "rails_helper"

RSpec.describe Campbooks::Now::SegmentRing, type: :component do
  def render_ring(**opts)
    ApplicationController.render(described_class.new(**{ segment: :mail, label: "Mail", count: 3 }.merge(opts)), layout: false)
  end

  it "renders a link to the segment when given an href" do
    html = render_ring(href: "/now?segment=mail")
    expect(html).to include("<a").and include('href="/now?segment=mail"')
    expect(html).to include("Mail")
  end

  it "renders a button (not a link) for the overlay-style Docs ring with no href" do
    html = render_ring(segment: :docs, label: "Docs", href: nil)
    expect(html).to include("<button").and include('type="button"')
  end

  it "shows the count badge and the Ember ring in the default state" do
    html = render_ring(count: 3)
    expect(html).to include("bg-ember-gradient").and include("shadow-ember")
    expect(html).to include(">3<")
  end

  it "emphasises the label and fills the disc when active" do
    html = render_ring(active: true)
    expect(html).to include("bg-secondary")     # inner disc filled
    expect(html).to include("font-semibold")     # label in full ink
  end

  it "renders the done state (gray ring, no glow, a check) when the count is zero" do
    html = render_ring(count: 0)
    expect(html).to include("bg-border")         # gray ring
    expect(html).not_to include("shadow-ember")  # no glow
    expect(html).to include("M20 6 9 17l-5-5")   # the check glyph
  end

  it "abbreviates large counts" do
    expect(render_ring(count: 1200)).to include("1.2k")
  end
end
