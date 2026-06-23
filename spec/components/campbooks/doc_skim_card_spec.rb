require "rails_helper"

# The reclassify ("Re-file as") picker must list EVERY workspace DocumentType. Types
# created via the setup wizard, the AI analyzer, or onboarding carry no `category`, so
# grouping strictly by DocumentType::CATEGORIES silently dropped them and left the
# picker empty for nearly every real workspace (only the seeded demo set has
# categories). These pin grouped_options: categorised types group, the rest still show.
RSpec.describe Campbooks::DocSkimCard, type: :component do
  # Duck-typed stand-in for a DocumentType (the component only reads id/name/category).
  Type = Struct.new(:id, :name, :category)

  def render_card(document_types:, type_id: nil)
    ApplicationController.render(
      described_class.new(
        document_id: 1, category: "other", display_title: "Doc",
        type_id: type_id, document_types: document_types
      ),
      layout: false
    )
  end

  # The reclassify <select> only — the card also renders inline-edit <select>s.
  def picker(html)
    html[%r{<select[^>]*doc-skim-reclassify-select.*?</select>}m].to_s
  end

  it "lists category-less types instead of dropping them (the regression)" do
    types = [ Type.new(11, "my_custom_type", nil), Type.new(12, "scanned_thing", "") ]

    select = picker(render_card(document_types: types))

    expect(select).to include(">My custom type<")
    expect(select).to include(">Scanned thing<")
    expect(select).not_to include("<optgroup") # all uncategorised → a flat list
  end

  it "groups types that do have a recognised category" do
    types = [ Type.new(1, "expense_invoice", "accounting"), Type.new(4, "contract", "legal") ]

    select = picker(render_card(document_types: types, type_id: 1))

    expect(select).to include('<optgroup label="Accounting">')
    expect(select).to include('<optgroup label="Legal">')
    expect(select).to include('<option value="1" selected>Expense invoice</option>')
  end

  it "keeps categorised groups and files the rest under Unclassified" do
    types = [ Type.new(1, "expense_invoice", "accounting"), Type.new(11, "my_custom_type", nil) ]

    select = picker(render_card(document_types: types))

    expect(select).to include('<optgroup label="Accounting">')
    expect(select).to include('<optgroup label="Unclassified">')
    expect(select).to include(">My custom type<")
  end

  it "shows a disabled placeholder (not a blank box) when the workspace has no types" do
    select = picker(render_card(document_types: []))

    expect(select).to include("No document types yet")
    expect(select).to include('disabled')
    expect(select).not_to include("<optgroup")
  end
end
