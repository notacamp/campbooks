# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::PlainText do
  it "drops style/script content that a bare strip_tags would leak as text" do
    html = "<style>div.zm p.MsoNormal { margin: 0cm }</style>" \
           "<p>Please send the signed contract.</p><script>var x = 1</script>"

    text = described_class.of(html)

    expect(text).to eq("Please send the signed contract.")
  end

  it "strips quoted reply history and its attribution from an HTML reply" do
    html = <<~HTML
      <div>Thanks — I will send it today.</div>
      <div>On Tue, Jul 1, 2026 at 9:00 AM Ana wrote:</div>
      <blockquote>Please send the signed contract back to us.</blockquote>
    HTML

    text = described_class.of(html)

    expect(text).to include("I will send it today")
    expect(text).not_to include("signed contract")
    expect(text).not_to include("wrote:")
  end

  it "keeps quoted history when strip_quotes is false" do
    html = "<div>Top reply</div><blockquote>Original ask</blockquote>"

    expect(described_class.of(html, strip_quotes: false)).to include("Original ask")
  end

  it "plain-text bodies drop the >-quoted history" do
    raw = "I will handle it.\n> Please send the file\n> by Friday"

    expect(described_class.of(raw)).to eq("I will handle it.")
  end

  it "blank input returns an empty string" do
    expect(described_class.of(nil)).to eq("")
    expect(described_class.of("   ")).to eq("")
  end
end
