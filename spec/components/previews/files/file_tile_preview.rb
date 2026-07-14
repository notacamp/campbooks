# frozen_string_literal: true

module Files
  # Preview for a file tile (the grid-view item). The grid wrapper mirrors the
  # Files page's column classes so the tile is shown at a realistic width.
  class FileTilePreview < Lookbook::Preview
    def default
      render_with_template(locals: { docs: sample_docs, folders: MailFolder.ordered.limit(5).to_a })
    end

    private

    # A handful of real documents when the dev DB has them (their thumbnails
    # render), padded with an unsaved fallback doc (renders the icon tile).
    def sample_docs
      docs = Document.where(source: :manual_upload).limit(4).to_a
      docs << Document.new(id: 0, source: :manual_upload, created_at: Time.current,
                           metadata: { "title" => "Quarterly report.pdf" })
      docs
    end
  end
end
