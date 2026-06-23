class CalendarAccountUser < ApplicationRecord
  belongs_to :calendar_account
  belongs_to :user

  validates :user_id, uniqueness: { scope: :calendar_account_id }

  # Assignable sharing roles layered over the boolean flags (mirror of
  # EmailAccountUser). `can_write` — create/edit/delete/RSVP — is the calendar
  # analogue of email's `can_send`. The owner row is tracked separately.
  ROLE_FLAGS = {
    "viewer"  => { can_read: true, can_write: false, can_manage: false },
    "editor"  => { can_read: true, can_write: true,  can_manage: false },
    "manager" => { can_read: true, can_write: true,  can_manage: true }
  }.freeze

  ROLES = ROLE_FLAGS.keys.freeze

  # Current role label, derived from the flags (or "owner" for the owner row).
  def role
    return "owner" if owner?
    return "manager" if can_manage?
    return "editor" if can_write?

    "viewer"
  end

  # Assign all three flags from a role name; ignores unknown roles so a crafted
  # value can't silently clear read access.
  def role=(value)
    flags = ROLE_FLAGS[value.to_s]
    assign_attributes(flags) if flags
  end
end
