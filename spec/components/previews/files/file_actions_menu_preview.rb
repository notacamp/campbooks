# frozen_string_literal: true

module Files
  # Preview for the per-file kebab menu (open / download / move / delete). Rendered
  # open is not possible without JS, so the trigger is shown; click it in the
  # preview to expand.
  class FileActionsMenuPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::FileActionsMenu.new(doc: sample_doc, folders: MailFolder.ordered.limit(5).to_a, current_folder: nil))
    end

    private

    def sample_doc
      Document.where(source: :manual_upload).first ||
        Document.new(id: 0, source: :manual_upload, created_at: Time.current,
                     metadata: { "title" => "Report.pdf" })
    end
  end
end
