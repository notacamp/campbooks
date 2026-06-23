class SenderUnblockUnarchiveJob < ApplicationJob
  queue_as :default

  # Inverse of SenderBlockArchiveJob: when a sender is unblocked, move their
  # previously-archived mail back to the inbox so unblock (and block's Undo)
  # restores what blocking removed. acting_user_id scopes the unarchive to that
  # user's readable accounts.
  def perform(contact_id, acting_user_id = nil)
    contact = Contact.find_by(id: contact_id)
    return unless contact

    Current.acting_user = User.find_by(id: acting_user_id) if acting_user_id
    Contacts::ApplyUnblock.call(contact)
  end
end
