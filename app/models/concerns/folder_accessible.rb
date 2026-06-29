# frozen_string_literal: true

# Permission inheritance for content filed into folders (Files Phase 3). A
# record is visible to a user when it's **unfiled**, OR filed in at least one
# **open** folder, OR filed in a **restricted** folder the user can read.
# (Filing something only into restricted folders the user can't read hides it.)
# Open folders are the default, so this is a no-op until a folder is restricted.
module FolderAccessible
  extend ActiveSupport::Concern

  included do
    scope :accessible_to, ->(user) {
      next none unless user
      next all if user.admin?

      readable_restricted_ids = MailFolderUser.where(user: user, can_read: true)
        .joins(:mail_folder).where(mail_folders: { restricted: true }).pluck(:mail_folder_id)

      visible_ids = FolderMembership.where(folderable_type: name).joins(:mail_folder)
        .where("mail_folders.restricted = FALSE OR mail_folders.id IN (?)", readable_restricted_ids)
        .select(:folderable_id)
      filed_ids = FolderMembership.where(folderable_type: name).select(:folderable_id)

      where.not(id: filed_ids).or(where(id: visible_ids))
    }
  end
end
