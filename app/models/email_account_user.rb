class EmailAccountUser < ApplicationRecord
  belongs_to :email_account
  belongs_to :user

  validates :user_id, uniqueness: { scope: :email_account_id }

  # The three assignable sharing roles surfaced in the UI, layered over the
  # underlying boolean permission flags. The account creator's `owner` flag is
  # tracked separately and is not one of these roles.
  ROLE_FLAGS = {
    "viewer"       => { can_read: true, can_send: false, can_manage: false },
    "collaborator" => { can_read: true, can_send: true,  can_manage: false },
    "manager"      => { can_read: true, can_send: true,  can_manage: true }
  }.freeze

  ROLES = ROLE_FLAGS.keys.freeze

  # Current role label, derived from the flags (or "owner" for the owner row).
  def role
    return "owner" if owner?
    return "manager" if can_manage?
    return "collaborator" if can_send?

    "viewer"
  end

  # Assign all three flags from a role name; ignores unknown roles so a crafted
  # value can't, e.g., silently clear read access.
  def role=(value)
    flags = ROLE_FLAGS[value.to_s]
    assign_attributes(flags) if flags
  end
end
