require "rails_helper"

RSpec.describe Campbooks::Divider, type: :component do
  def render_for(**args)
    ApplicationController.render(described_class.new(**args), layout: false)
  end

  it "renders a labeled divider whose wrapper positions the line" do
    html = render_for(label: "or")
    expect(html).to include("relative")
    expect(html).to include("absolute inset-0")
    expect(html).to include(">or<")
  end

  # Regression: a caller's `class:` used to clobber the wrapper's classes,
  # dropping `relative` — so the `absolute inset-0` line escaped to the nearest
  # positioned ancestor and overlaid (and blocked clicks on) everything below
  # the divider, e.g. the OAuth buttons on the sign-in page.
  it "keeps `relative` on the wrapper when a custom class is passed" do
    html = render_for(label: "or", class: "my-6")
    expect(html).to include("relative")
    expect(html).to include("my-6")
  end

  it "renders an <hr> when unlabeled, merging any custom class" do
    html = render_for(class: "mt-4")
    expect(html).to include("<hr")
    expect(html).to include("border-border")
    expect(html).to include("mt-4")
  end

  it "backs the label with the canvas by default" do
    expect(render_for(label: "or")).to include("bg-background")
  end

  # Inside a raised card (e.g. the auth screens) the label must match the card,
  # not the darker page background, or a seam shows behind it in dark mode.
  it "backs the label with the card surface when surface: :card" do
    html = render_for(label: "or", surface: :card)
    expect(html).to include("bg-card")
    expect(html).not_to include("bg-background")
  end
end
