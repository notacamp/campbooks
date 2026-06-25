require "rails_helper"

RSpec.describe Campbooks::Icon, type: :component do
  def render_icon(*args, **opts)
    ApplicationController.render(described_class.new(*args, **opts), layout: false)
  end

  it "renders an inline svg for a known name" do
    html = render_icon("star")
    expect(html).to include("<svg")
    expect(html).to include('stroke="currentColor"')
  end

  it "applies the css_class to the svg" do
    expect(render_icon("folder", css_class: "w-4 h-4")).to include('class="w-4 h-4"')
  end

  it "falls back to the default folder glyph for an unknown name" do
    expect(render_icon("nope")).to eq(render_icon(Campbooks::Icon::DEFAULT))
  end

  describe ".for_folder_name" do
    it "maps system folder names to glyphs (case-insensitive)" do
      expect(described_class.for_folder_name("Sent")).to eq("paper-airplane")
      expect(described_class.for_folder_name("inbox")).to eq("inbox")
    end

    it "defaults to the folder glyph for unknown names" do
      expect(described_class.for_folder_name("Receipts")).to eq(Campbooks::Icon::DEFAULT)
    end
  end
end
