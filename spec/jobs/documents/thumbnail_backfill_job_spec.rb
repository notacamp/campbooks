require "rails_helper"

# The one-shot upgrade sweep. Contract: it targets exactly the PDFs/images that
# have no thumbnail yet (skipping already-thumbed and non-renderable types) and
# runs the generator inline — one job, never a per-document fan-out.
RSpec.describe Documents::ThumbnailBackfillJob do
  def doc_with(content_type:, filename:)
    doc = create(:document)
    doc.original_file.attach(io: StringIO.new("x"), filename: filename, content_type: content_type)
    doc
  end

  it "generates for unthumbed PDFs/images only" do
    pdf   = create(:document) # factory attaches a PDF
    image = doc_with(content_type: "image/png", filename: "scan.png")
    zip   = doc_with(content_type: "application/zip", filename: "a.zip")
    done  = create(:document)
    done.thumbnail.attach(io: StringIO.new("jpg"), filename: "thumbnail.jpg", content_type: "image/jpeg")

    seen = []
    allow(Documents::ThumbnailGenerator).to receive(:new) do |doc|
      seen << doc.id
      instance_double(Documents::ThumbnailGenerator, call: true)
    end

    described_class.perform_now

    expect(seen).to match_array([ pdf.id, image.id ])
    expect(seen).not_to include(zip.id, done.id)
  end
end
