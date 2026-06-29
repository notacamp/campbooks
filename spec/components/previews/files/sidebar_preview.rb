# frozen_string_literal: true

module Files
  # Preview for the Files folder rail. Uses real workspace folders when present,
  # otherwise a small in-memory tree so the nesting + counts are visible.
  class SidebarPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::Sidebar.new(folders: folders, current_folder: folders.first, counts: counts))
    end

    def empty
      render(Campbooks::Files::Sidebar.new(folders: [], current_folder: nil, counts: {}))
    end

    private

    def folders
      real = MailFolder.order(:position, :name).limit(6).to_a
      return real if real.any?

      [
        MailFolder.new(id: 1, name: "Contracts", position: 0),
        MailFolder.new(id: 2, name: "Invoices", position: 1),
        MailFolder.new(id: 3, name: "2026", parent_id: 2, position: 0)
      ]
    end

    def counts
      folders.each_with_object({}) { |f, h| h[f.id] = (f.id * 2) % 5 }
    end
  end
end
