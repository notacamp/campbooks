class MailFolderUser < ApplicationRecord
  belongs_to :mail_folder
  belongs_to :user

  validates :user_id, uniqueness: { scope: :mail_folder_id }

  # Sharing roles surfaced in the UI, layered over the boolean flags. The folder
  # creator's `owner` flag is tracked separately and isn't one of these roles.
  # Mirrors EmailAccountUser (calendar's can_write maps to email's can_send).
  ROLE_FLAGS = {
    "viewer"  => { can_read: true, can_write: false, can_manage: false },
    "editor"  => { can_read: true, can_write: true,  can_manage: false },
    "manager" => { can_read: true, can_write: true,  can_manage: true }
  }.freeze

  ROLES = ROLE_FLAGS.keys.freeze

  def role
    return "owner" if owner?
    return "manager" if can_manage?
    return "editor" if can_write?

    "viewer"
  end

  # Assign all flags from a role name; ignores unknown roles so a crafted value
  # can't silently clear read access.
  def role=(value)
    flags = ROLE_FLAGS[value.to_s]
    assign_attributes(flags) if flags
  end
end
