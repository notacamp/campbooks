# frozen_string_literal: true

module Files
  # Preview for a file card (the mobile list item).
  class FileCardPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::FileCard.new(doc: sample_doc, folders: MailFolder.ordered.limit(5).to_a, current_folder: nil))
    end

    private

    def sample_doc
      Document.where(source: :manual_upload).first ||
        Document.new(id: 0, source: :manual_upload, created_at: Time.current,
                     metadata: { "title" => "Quarterly report.pdf" })
    end
  end
end
