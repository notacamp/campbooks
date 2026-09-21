# frozen_string_literal: true

require "rails_helper"

RSpec.describe Api::App::Settings::MemoryEntrySerializer do
  Entry = Struct.new(:id, :facet, :sentence, :origin, :origin_detail, :actions, :form_path)

  it "exposes plain + spans (with bold) and NO html field" do
    sentence = Scout::Memory::Sentence.parse("File **EDP** under Utilities.")
    entry = Entry.new("e1", "rules", sentence, "taught", nil, [], "/x")

    json = described_class.new(entry).as_json

    expect(json[:sentence]).to eq(
      plain: "File EDP under Utilities.",
      spans: [
        { text: "File ", bold: false },
        { text: "EDP", bold: true },
        { text: " under Utilities.", bold: false }
      ]
    )
    # No `html` field — it was misleading (just plain text) and an XSS sink risk.
    expect(json[:sentence]).not_to have_key(:html)
  end

  it "wraps a plain-string sentence in a single non-bold span" do
    entry = Entry.new("e2", "tags", "just text", "default", nil, [], nil)

    json = described_class.new(entry).as_json

    expect(json[:sentence]).to eq(plain: "just text", spans: [ { text: "just text", bold: false } ])
  end
end
