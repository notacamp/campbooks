# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::ScoutNoteRow, type: :component do
  it "renders Scout, the AI tag, and the read on the glass surface" do
    html = ApplicationController.render(
      described_class.new(text: "Two invoices are unpaid, €612.00 in total."), layout: false
    )
    expect(html).to include("scout-glass")
    expect(html).to include("Scout")
    expect(html).to include("AI")
    expect(html).to include("Two invoices are unpaid, €612.00 in total.")
  end

  it "yields a trailing action slot (the review / connect button)" do
    harness = stub_const("ScoutNoteRowSlotHarness", Class.new(Campbooks::Base) do
      def view_template
        render Campbooks::ScoutNoteRow.new(text: "Nothing needs you.") { plain "REVIEW_BUTTON_SLOT" }
      end
    end)
    html = ApplicationController.render(harness.new, layout: false)
    expect(html).to include("REVIEW_BUTTON_SLOT")
  end
end
