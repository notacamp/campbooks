# frozen_string_literal: true

module Documents
  # Renders a document's grid-view thumbnail: the first PDF page, or a bounded
  # copy of an image, as a JPEG attached to Document#thumbnail. Runs on the
  # worker (GenerateThumbnailJob / ThumbnailBackfillJob), never on a request.
  #
  # Uses ImageMagick + Ghostscript directly — the toolchain the document
  # pipeline already requires (Pdf::ImageConverter, the statement parser; the
  # Dockerfile installs and policy-patches both). Active Storage's own
  # variant/preview machinery is unusable here: it attaches processed images to
  # bigint-keyed VariantRecord/Blob rows, which this schema's uuid
  # `active_storage_attachments.record_id` cannot reference.
  #
  # Best-effort by design: any render failure (corrupt file, missing decoder)
  # logs and leaves the thumbnail unattached, so the tile keeps its type icon.
  class ThumbnailGenerator
    # Bounded so a 5-column grid tile still renders sharp at 2x DPR; ~72dpi of
    # an A4 page would be blurry, so PDFs decode at 144dpi before the resize.
    MAX_EDGE = 640
    PDF_DENSITY = 144
    JPEG_QUALITY = 85

    def initialize(document)
      @document = document
    end

    # Returns true when a thumbnail is attached on exit (including a prior one).
    def call
      return true if @document.thumbnail.attached?
      return false unless @document.thumbnailable? && @document.original_file.attached?

      data = render_jpeg
      return false if data.nil?

      @document.thumbnail.attach(
        io: StringIO.new(data),
        filename: "thumbnail.jpg",
        content_type: "image/jpeg"
      )
      true
    rescue StandardError => e
      Rails.logger.warn("[Documents::ThumbnailGenerator] #{@document.id}: #{e.class} #{e.message}")
      false
    end

    private

    def render_jpeg
      extension = @document.pdf? ? ".pdf" : File.extname(@document.original_file.filename.to_s)
      Tempfile.create([ "thumb_src", extension ], binmode: true) do |src|
        src.write(@document.original_file.download)
        src.flush

        Tempfile.create([ "thumb_out", ".jpg" ], binmode: true) do |out|
          # Page selection MUST use the input bracket syntax ("input.pdf[0]") and
          # -density must precede the input so the PDF decodes at PDF_DENSITY
          # rather than being upscaled from 72dpi — see Ai::BankStatementParser.
          MiniMagick.convert do |convert|
            convert.density(PDF_DENSITY) if @document.pdf?
            convert << "#{src.path}[0]"
            convert.auto_orient
            convert.thumbnail("#{MAX_EDGE}x#{MAX_EDGE}>")
            # PDFs and transparent images render over white, not black.
            convert.background("white").alpha("remove").alpha("off")
            convert.quality(JPEG_QUALITY)
            convert << out.path
          end

          data = File.binread(out.path)
          data.empty? ? nil : data
        end
      end
    end
  end
end
