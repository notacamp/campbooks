# frozen_string_literal: true

# A local, no-network mail client for the seeded demo account (refresh_token ==
# EmailAccount::DEMO_TOKEN). It exposes the same interface as the real provider
# clients (Zoho, Google, IMAP, Microsoft) so Tools::Archive, Tools::Unarchive,
# Tools::Snooze, Tools::Unsnooze, and Emails::InboxFolders work unchanged.
#
# Folder ids are stable string constants that the seed writes onto every demo
# EmailMessage so that inbox/archive/snoozed filtering works correctly from the
# first page load.
#
# move_to_folder is intentionally a no-op: every Tools::* class calls it for
# the provider-side move and then does its own update_all(provider_folder_id:)
# for the local persistence, so the local record is always accurate without any
# network call.
class DemoMailClient
  INBOX_FOLDER_ID    = "demo-inbox"
  ARCHIVE_FOLDER_ID  = "demo-archive"
  SNOOZED_FOLDER_ID  = "demo-snoozed"

  FOLDERS = [
    { "folderId" => INBOX_FOLDER_ID,   "folderName" => "Inbox" },
    { "folderId" => ARCHIVE_FOLDER_ID, "folderName" => "Archive" },
    { "folderId" => SNOOZED_FOLDER_ID, "folderName" => "Snoozed" }
  ].freeze

  def list_folders
    FOLDERS
  end

  def inbox_folder_id
    INBOX_FOLDER_ID
  end

  def archive_folder_id
    ARCHIVE_FOLDER_ID
  end

  def snoozed_folder_id
    SNOOZED_FOLDER_ID
  end

  # Provider-side move is a no-op for demo messages: Tools::* already calls
  # update_all(provider_folder_id:) after this, so local persistence is correct.
  def move_to_folder(_message_ids, _folder_id)
    # intentionally empty
  end
end
