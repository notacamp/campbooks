module Pdf
  class ImageConverter
    A4_WIDTH_PT = 595    # A4 width in points (72 DPI)
    A4_HEIGHT_PT = 842   # A4 height in points (72 DPI)
    MARGIN = 36          # 0.5 inch margin

    def initialize(document)
      @document = document
    end

    def call
      blob = @document.original_file.blob

      Tempfile.create([ "image", extension_for(blob.content_type) ]) do |temp_image|
        temp_image.binmode
        temp_image.write(blob.download)
        temp_image.rewind

        # Auto-orient and optimize the image
        image = MiniMagick::Image.open(temp_image.path)
        image.auto_orient
        image.strip # Remove EXIF data

        # Generate PDF
        pdf_content = generate_pdf(image.path)

        @document.processed_pdf.attach(
          io: StringIO.new(pdf_content),
          filename: @document.canonical_filename || "converted.pdf",
          content_type: "application/pdf"
        )
      end
    end

    private

    def generate_pdf(image_path)
      usable_width = A4_WIDTH_PT - (2 * MARGIN)
      usable_height = A4_HEIGHT_PT - (2 * MARGIN)

      pdf = Prawn::Document.new(
        page_size: "A4",
        margin: MARGIN
      )

      pdf.image(image_path, fit: [ usable_width, usable_height ], position: :center, vposition: :center)
      pdf.render
    end

    def extension_for(content_type)
      case content_type
      when "image/jpeg" then ".jpg"
      when "image/png" then ".png"
      when "image/webp" then ".webp"
      when "image/heic" then ".heic"
      else ".jpg"
      end
    end
  end
end
