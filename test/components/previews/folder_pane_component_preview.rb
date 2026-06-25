# frozen_string_literal: true

class FolderPaneComponentPreview < ViewComponent::Preview
  # The expanded pane with system folders above a few custom folders.
  def default
    render(Campbooks::FolderPane.new(
      system_folders: [
        { id: nil, name: "Inbox", count: 12 },
        { id: "sent", name: "Sent", count: 0 },
        { id: "drafts", name: "Drafts", count: 2 },
        { id: "archive", name: "Archive", count: 0 },
        { id: "spam", name: "Spam", count: 0 },
        { id: "trash", name: "Trash", count: 0 }
      ],
      custom_folders: [
        MailFolder.new(name: "Receipts", icon: "currency-dollar"),
        MailFolder.new(name: "Clients", icon: "briefcase"),
        MailFolder.new(name: "Travel", icon: "map-pin"),
        MailFolder.new(name: "Personal", icon: "heart")
      ],
      current_folder: nil
    ))
  end
end
