module Contacts
  # Side effect of unblocking a sender: move their previously-archived mail back
  # to the inbox folder, so it returns to the inbox, Skim, and the feed. The
  # inverse of Contacts::ApplyBlock. Scoped to the acting user's readable
  # accounts. Already-in-inbox mail is a no-op (BulkUnarchive only moves what's
  # archived).
  class ApplyUnblock
    def self.call(contact)
      return { unarchived_count: 0 } unless contact

      ids = EmailMessage.accessible_to(Current.user)
                        .where(contact_id: contact.id)
                        .pluck(:id)
      return { unarchived_count: 0 } if ids.empty?

      Tools::BulkUnarchive.call("email_ids" => ids)
    end
  end
end
