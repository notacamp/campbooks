require "rails_helper"

# The grid-view thumbnail renderer. Contracts: idempotent (an attached thumbnail
# short-circuits), gated to PDFs/images, and best-effort (a render failure never
# raises out — the tile keeps its icon). Real ImageMagick renders only run where
# the binary exists (dev/prod images ship it; the CI runner may not).
RSpec.describe Documents::ThumbnailGenerator do
  def doc_with(content_type:, filename:, data: "x")
    doc = create(:document)
    doc.original_file.attach(io: StringIO.new(data), filename: filename, content_type: content_type)
    doc
  end

  it "skips non-thumbnailable types without invoking ImageMagick" do
    doc = doc_with(content_type: "application/zip", filename: "a.zip")
    expect(MiniMagick).not_to receive(:convert)
    expect(described_class.new(doc).call).to be false
    expect(doc.thumbnail).not_to be_attached
  end

  it "returns true without re-rendering when a thumbnail is already attached" do
    doc = create(:document)
    doc.thumbnail.attach(io: StringIO.new("jpg"), filename: "thumbnail.jpg", content_type: "image/jpeg")
    expect(MiniMagick).not_to receive(:convert)
    expect(described_class.new(doc).call).to be true
  end

  it "logs and returns false when the render fails (corrupt file), leaving the icon fallback" do
    doc = create(:document) # factory attaches fake, unparseable PDF bytes
    allow(MiniMagick).to receive(:convert).and_raise(MiniMagick::Error, "no decode delegate")
    expect(described_class.new(doc).call).to be false
    expect(doc.thumbnail).not_to be_attached
  end

  context "with ImageMagick available", if: system("magick -version >/dev/null 2>&1 || convert -version >/dev/null 2>&1") do
    it "renders a real PDF's first page to an attached JPEG" do
      pdf = Prawn::Document.new { |p| p.text "Invoice 2026-031", size: 20 }.render
      doc = doc_with(content_type: "application/pdf", filename: "invoice.pdf", data: pdf)

      expect(described_class.new(doc).call).to be true
      expect(doc.thumbnail).to be_attached
      expect(doc.thumbnail.content_type).to eq("image/jpeg")
      expect(doc.thumbnail.byte_size).to be > 1_000
    end

    it "renders an image to a bounded JPEG" do
      png = Tempfile.create([ "px", ".png" ], binmode: true) do |f|
        system("magick", "-size", "900x700", "xc:#74a9d0", f.path) ||
          system("convert", "-size", "900x700", "xc:#74a9d0", f.path)
        File.binread(f.path)
      end
      doc = doc_with(content_type: "image/png", filename: "photo.png", data: png)

      expect(described_class.new(doc).call).to be true
      expect(doc.thumbnail).to be_attached
    end
  end
end
