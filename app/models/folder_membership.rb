class FolderMembership < ApplicationRecord
  # Places a piece of content (a Document now; emails / reminders later) into a
  # custom folder — the Stage 3 "filesystem" layer over the folder tree.
  belongs_to :mail_folder
  belongs_to :folderable, polymorphic: true

  validates :folderable_id, uniqueness: { scope: [ :mail_folder_id, :folderable_type ] }
end
