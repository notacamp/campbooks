class SenderBlockArchiveJob < ApplicationJob
  queue_as :default

  # Archives a freshly-blocked sender's existing mail in the background so the
  # block action returns immediately. acting_user_id scopes the archive to that
  # user's readable accounts (Tools::BulkArchive reads Current.user).
  def perform(contact_id, acting_user_id = nil)
    contact = Contact.find_by(id: contact_id)
    return unless contact

    Current.acting_user = User.find_by(id: acting_user_id) if acting_user_id
    Contacts::ApplyBlock.call(contact)
  end
end
