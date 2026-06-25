# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::SkimScope do
  let(:workspace) { create(:workspace) }

  def reattach(doc, filename:, content_type:)
    doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: content_type)
    doc
  end

  it "includes reviewable documents (the factory default is a PDF)" do
    doc = create(:document, :in_review, workspace: workspace)
    expect(described_class.for(workspace).map(&:id)).to include(doc.id)
  end

  it "excludes non-document attachments — calendar invites, archives, raw emails, DMARC reports" do
    ics  = reattach(create(:document, :in_review, workspace: workspace), filename: "invite.ics", content_type: "text/calendar")
    zip  = reattach(create(:document, :in_review, workspace: workspace), filename: "a.zip", content_type: "application/zip")
    eml  = reattach(create(:document, :in_review, workspace: workspace), filename: "m.eml", content_type: "message/rfc822")
    dmarc = reattach(create(:document, :in_review, workspace: workspace), filename: "r.xml", content_type: "application/xhtml+xml")
    keep = create(:document, :in_review, workspace: workspace) # PDF

    ids = described_class.for(workspace).map(&:id)
    expect(ids).to include(keep.id)
    expect(ids).not_to include(ics.id, zip.id, eml.id, dmarc.id)
  end

  it "orders most-uncertain first (lowest confidence, NULLs first)" do
    high = create(:document, :in_review, workspace: workspace, ai_confidence_score: 0.9)
    low  = create(:document, :in_review, workspace: workspace, ai_confidence_score: 0.1)
    none = create(:document, :in_review, workspace: workspace, ai_confidence_score: nil)

    expect(described_class.for(workspace).map(&:id)).to eq([ none.id, low.id, high.id ])
  end

  it "returns nothing for a nil workspace" do
    expect(described_class.for(nil)).to eq(Document.none)
  end
end
