# frozen_string_literal: true

module Files
  # Preview for a single file table row. Rendered inside a minimal table so the
  # cells line up the way they do on the Files page.
  class FileRowPreview < Lookbook::Preview
    # @param current_folder toggle [Boolean] show the "remove from folder" action
    def default(current_folder: false)
      folder = MailFolder.first
      render_with_template(locals: {
        doc: sample_doc,
        folders: MailFolder.order(:position).limit(5).to_a,
        current_folder: current_folder ? folder : nil
      })
    end

    private

    def sample_doc
      Document.where(source: :manual_upload).first ||
        Document.new(id: 0, source: :manual_upload, created_at: Time.current,
                     metadata: { "title" => "Quarterly report.pdf" })
    end
  end
end
