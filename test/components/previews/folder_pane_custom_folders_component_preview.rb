# frozen_string_literal: true

class FolderPaneCustomFoldersComponentPreview < ViewComponent::Preview
  # A nested tree (Clients under Work) plus a top-level folder, each with its
  # hover "edit" affordance + per-folder edit dialog.
  def default
    render(Campbooks::FolderPaneCustomFolders.new(
      custom_folders: [
        MailFolder.new(id: 1, name: "Work", icon: "briefcase"),
        MailFolder.new(id: 2, name: "Clients", icon: "user", parent_id: 1),
        MailFolder.new(id: 3, name: "Receipts", icon: "currency-dollar")
      ],
      current_folder: "Receipts"
    ))
  end
end
