require "rails_helper"

# Campbooks::Files::FileTile is the grid-view unit. The contract worth pinning:
# a document with a generated thumbnail renders it as a lazy <img>; one without
# (not yet generated, or a type the pipeline can't render) falls back to the
# shared type icon + kind label — the grid never breaks. Appearance is covered
# by the Lookbook preview; the frame-escape rule is pinned in frame_escape_spec.
RSpec.describe Campbooks::Files::FileTile, type: :component do
  def render_tile(doc)
    ApplicationController.render(described_class.new(doc: doc), layout: false)
  end

  def doc_with(content_type:, filename:)
    doc = create(:document)
    doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: content_type)
    doc
  end

  it "renders a lazy thumbnail image when one has been generated" do
    doc = create(:document)
    doc.thumbnail.attach(io: StringIO.new("jpg"), filename: "thumbnail.jpg", content_type: "image/jpeg")
    html = render_tile(doc)
    expect(html).to include("<img")
    expect(html).to include('loading="lazy"')
  end

  it "falls back to the type icon and kind label without a thumbnail" do
    html = render_tile(doc_with(content_type: "application/zip", filename: "archive.zip"))
    expect(html).not_to include("<img")
    expect(html).to include("ZIP")
  end

  it "shows the name and meta line" do
    doc = doc_with(content_type: "application/zip", filename: "archive.zip")
    html = render_tile(doc)
    expect(html).to include(doc.display_title)
    expect(html).to include("1 Byte") # human size of the 1-byte StringIO
  end
end
