# frozen_string_literal: true

class FolderBottomSheetComponentPreview < ViewComponent::Preview
  # The sheet rendered with a mix of system and custom folders. In the real
  # inbox the "Folders" trigger in the chip bar opens this with
  # data-action="folder-bottom-sheet#open"; in Lookbook the sheet panel is
  # in the DOM and can be exercised via the Stimulus controller in the preview
  # iframe (call window.Stimulus.controllers[...].open() in the console).
  def default
    render(Campbooks::FolderBottomSheet.new(
      system_folders: [
        { id: nil,       name: "Inbox",   count: 5  },
        { id: "sent",    name: "Sent",    count: 0  },
        { id: "drafts",  name: "Drafts",  count: 1  },
        { id: "archive", name: "Archive", count: 0  },
        { id: "spam",    name: "Spam",    count: 0  },
        { id: "trash",   name: "Trash",   count: 0  }
      ],
      custom_folders: [
        MailFolder.new(name: "Receipts",  icon: "currency-dollar"),
        MailFolder.new(name: "Clients",   icon: "briefcase"),
        MailFolder.new(name: "Travel",    icon: "map-pin"),
        MailFolder.new(name: "Personal",  icon: "heart")
      ],
      current_folder: nil
    ))
  end

  # Sheet with one custom folder active — verifies the accent highlight.
  def active_folder
    render(Campbooks::FolderBottomSheet.new(
      system_folders: [
        { id: nil,       name: "Inbox",   count: 3  },
        { id: "sent",    name: "Sent",    count: 0  },
        { id: "archive", name: "Archive", count: 0  }
      ],
      custom_folders: [
        MailFolder.new(name: "Receipts", icon: "currency-dollar"),
        MailFolder.new(name: "Clients",  icon: "briefcase")
      ],
      current_folder: "Receipts"
    ))
  end

  # Sheet with no custom folders — renders system rows + the New folder button only.
  def no_custom_folders
    render(Campbooks::FolderBottomSheet.new(
      system_folders: [
        { id: nil,       name: "Inbox",   count: 0  },
        { id: "sent",    name: "Sent",    count: 0  },
        { id: "drafts",  name: "Drafts",  count: 0  },
        { id: "archive", name: "Archive", count: 0  }
      ],
      custom_folders: [],
      current_folder: nil
    ))
  end
end
