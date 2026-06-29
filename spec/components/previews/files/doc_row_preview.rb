# frozen_string_literal: true

module Files
  # Preview for an internal (authored) document list item.
  class DocRowPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::DocRow.new(doc: sample, folders: MailFolder.ordered.limit(5).to_a, current_folder: nil))
    end

    private

    def sample
      AuthoredDocument.first ||
        AuthoredDocument.new(id: "preview", title: "Project brief", updated_at: Time.current)
    end
  end
end
