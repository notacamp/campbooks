# frozen_string_literal: true

class FolderPaneCustomFoldersComponentPreview < ViewComponent::Preview
  # Custom folders with their hover "edit" affordance + per-folder edit dialogs.
  def default
    render(Campbooks::FolderPaneCustomFolders.new(
      custom_folders: [
        MailFolder.new(id: 1, name: "Receipts", icon: "currency-dollar"),
        MailFolder.new(id: 2, name: "Clients", icon: "briefcase")
      ],
      current_folder: "Receipts"
    ))
  end
end
