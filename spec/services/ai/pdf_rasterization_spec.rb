# frozen_string_literal: true

require "rails_helper"

# Real-ImageMagick regression coverage for the 2026-07-10 prod incident: page
# selection via Image#format + Image#page produced junk thumbnails on
# multipage PDFs (Image#page is canvas geometry, not a page selector), so the
# model received unreadable images and correctly returned zero transactions.
# These examples run the actual convert tool and are skipped where ImageMagick
# (or its PDF delegate, ghostscript) is unavailable — e.g. default CI runners.
RSpec.describe "PDF page rasterization", type: :service do
  def imagemagick_with_pdf_support?
    require "mini_magick"
    out = Tempfile.create([ "probe", ".jpg" ]) do |t|
      MiniMagick.convert do |c|
        c << Rails.root.join("spec/fixtures/files/bank_statements/two_pages.pdf").to_s + "[0]"
        c << t.path
      end
      File.size(t.path)
    end
    out.positive?
  rescue StandardError
    false
  end

  let(:pdf_data) { Rails.root.join("spec/fixtures/files/bank_statements/two_pages.pdf").binread }

  describe Ai::BankStatementParser, "#rasterize_page" do
    let(:workspace) { Workspace.create!(name: "Raster WS") }
    let(:document) do
      doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                      review_status: :pending, source: :manual_upload)
      doc.original_file.attach(io: StringIO.new(pdf_data), filename: "s.pdf", content_type: "application/pdf")
      doc.save!
      doc
    end

    it "renders each page as a distinct, real-sized JPEG" do
      skip "ImageMagick with PDF delegate not available" unless imagemagick_with_pdf_support?

      parser = described_class.new(document)
      page0 = parser.send(:rasterize_page, pdf_data, 0)
      page1 = parser.send(:rasterize_page, pdf_data, 1)

      expect(page0).to be_present
      expect(page1).to be_present
      expect(Base64.decode64(page0[:data]).bytesize).to be > described_class::MIN_RENDER_BYTES
      expect(Base64.decode64(page1[:data]).bytesize).to be > described_class::MIN_RENDER_BYTES
      # Different pages must produce different pixels — identical output was
      # the incident's signature (two copies of the same junk thumbnail).
      expect(page0[:data]).not_to eq(page1[:data])
    end

    it "counts pages via the frame list, not identify's selected-list %n" do
      skip "ImageMagick with PDF delegate not available" unless imagemagick_with_pdf_support?

      parser = described_class.new(document)
      # The old `identify "path[0]"` + %n approach always returned 1 for
      # multipage PDFs, so only the cover page was rasterized.
      expect(parser.send(:page_count, pdf_data)).to eq(2)
    end

    it "returns nil (failed render) for an out-of-range page instead of junk" do
      skip "ImageMagick with PDF delegate not available" unless imagemagick_with_pdf_support?

      parser = described_class.new(document)
      expect(parser.send(:rasterize_page, pdf_data, 7)).to be_nil
    end
  end

  describe Ai::Adapters::Openai, "#pdf_to_image" do
    it "renders the first page of a multipage PDF as a real-sized image part" do
      skip "ImageMagick with PDF delegate not available" unless imagemagick_with_pdf_support?

      adapter = described_class.new(api_key: "test-key")
      part = adapter.send(:pdf_to_image, Base64.strict_encode64(pdf_data), "application/pdf")

      expect(part[:type]).to eq("image_url")
      data_url = part.dig(:image_url, :url)
      jpeg = Base64.decode64(data_url.split("base64,").last)
      expect(jpeg.bytesize).to be > 10_240
    end
  end
end
