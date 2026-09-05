require "rails_helper"

RSpec.describe Campbooks::HintTip, type: :component do
  def render_tip(...)
    ApplicationController.render(described_class.new(...), layout: false)
  end

  describe "live render (no preview)" do
    subject(:html) { render_tip }

    it "renders a div with id hint-tip" do
      expect(html).to include('id="hint-tip"')
    end

    it "has popover=manual" do
      expect(html).to include('popover="manual"')
    end

    it "is aria-hidden" do
      expect(html).to include('aria-hidden="true"')
    end

    it "has hint_target=tip on the outer element" do
      expect(html).to include('data-hint-target="tip"')
    end

    it "has a hint_target=label span" do
      expect(html).to include('data-hint-target="label"')
    end

    it "has a hint_target=keys span" do
      expect(html).to include('data-hint-target="keys"')
    end

    it "does not show is-on or is-preview classes" do
      expect(html).not_to include("is-on")
      expect(html).not_to include("is-preview")
    end
  end

  describe "preview render" do
    subject(:html) do
      render_tip(preview: { label: "Mark done", keys: %w[D] })
    end

    it "shows the label text" do
      expect(html).to include("Mark done")
    end

    it "renders one kbd per key" do
      expect(html.scan(/<kbd/).size).to eq(1)
      expect(html).to include("D")
    end

    it "has is-preview class" do
      expect(html).to include("is-preview")
    end

    it "does not have a popover attribute" do
      expect(html).not_to include("popover=")
    end

    context "with multiple keys" do
      subject(:html) do
        render_tip(preview: { label: "People", keys: %w[G P] })
      end

      it "renders two kbd elements" do
        expect(html.scan(/<kbd/).size).to eq(2)
      end
    end

    context "with no keys" do
      subject(:html) do
        render_tip(preview: { label: "Star Maya", keys: [] })
      end

      it "renders no kbd elements" do
        expect(html).not_to include("<kbd")
      end
    end
  end
end
