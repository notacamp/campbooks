require "rails_helper"

RSpec.describe Scout::Memory::Sentence do
  it "splits **bold** runs into spans" do
    spans = described_class.parse("File anything from **@edp.pt** under **Utilities**.").spans
    expect(spans).to eq([
      { text: "File anything from ", bold: false },
      { text: "@edp.pt", bold: true },
      { text: " under ", bold: false },
      { text: "Utilities", bold: true },
      { text: ".", bold: false }
    ])
  end

  it "handles a leading bold run and no markers" do
    expect(described_class.parse("**Hi** there").spans).to eq([ { text: "Hi", bold: true }, { text: " there", bold: false } ])
    expect(described_class.parse("plain").spans).to eq([ { text: "plain", bold: false } ])
  end

  it "exposes the plain text and is value-equal" do
    s = described_class.parse("a **b** c")
    expect(s.plain).to eq("a b c")
    expect(s).to eq(described_class.parse("a **b** c"))
  end

  it "leaves stray single asterisks alone" do
    expect(described_class.parse("2 * 3 = 6").plain).to eq("2 * 3 = 6")
  end
end
