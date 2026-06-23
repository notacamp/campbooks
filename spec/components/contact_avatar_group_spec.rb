require "rails_helper"

# Campbooks::ContactAvatarGroup is the participant facepile for a multi-sender
# thread. The contract worth pinning: it dedupes addresses, caps visible faces
# with a "+N" chip, and renders nothing when empty. Appearance (overlap, cut-out
# ring, dark mode) is covered by the Lookbook preview + Playwright.
RSpec.describe Campbooks::ContactAvatarGroup, type: :component do
  def render_for(participants, **opts)
    ApplicationController.render(described_class.new(participants: participants, **opts), layout: false)
  end

  def people(n)
    Array.new(n) { |i| { email: "p#{i}@example.com", contact_id: nil } }
  end

  it "renders one face per participant under the cap" do
    html = render_for(people(3), size: :xl, max: 3)
    expect(html).to include('data-testid="facepile"')
    expect(html.scan('relative rounded-full ring-2 ring-background').size).to eq(3)
    expect(html).not_to include(">+")
  end

  it "caps visible faces and shows a +N overflow chip" do
    html = render_for(people(6), size: :xl, max: 3)
    # 3 faces + 1 overflow chip, all carrying the cut-out ring
    expect(html.scan('ring-2 ring-background').size).to eq(4)
    expect(html).to include(">+3<")
    expect(html).to include('aria-label="3 more"')
  end

  it "dedupes addresses case-insensitively" do
    html = render_for([ { email: "Ann@x.com" }, { email: "ann@x.com" }, { email: "bob@y.com" } ], size: :md)
    expect(html.scan('relative rounded-full ring-2 ring-background').size).to eq(2)
  end

  it "renders nothing when there are no participants" do
    expect(render_for([], size: :md).strip).to be_empty
  end

  it "ignores blank addresses" do
    html = render_for([ { email: "" }, { email: nil }, { email: "real@x.com" } ], size: :md)
    expect(html.scan('relative rounded-full ring-2 ring-background').size).to eq(1)
  end

  it "stays neutral (never Ember/accent) by default" do
    html = render_for(people(2), size: :xl)
    expect(html).to include("bg-gray-200")
    expect(html).not_to match(/ember|accent-/)
  end
end
