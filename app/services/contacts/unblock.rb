module Contacts
  # Inverse of Contacts::Block: clear the block and bring the sender's archived
  # mail back to the inbox, so unblocking (and block's Undo) actually restores
  # what blocking removed. The unarchive runs in the background.
  module Unblock
    def self.call(contact, user: nil)
      return unless contact

      contact.unblock!
      SenderUnblockUnarchiveJob.perform_later(contact.id, user&.id)
    end
  end
end
