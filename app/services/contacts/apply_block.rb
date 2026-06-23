module Contacts
  # Side effect of blocking a sender: archive their existing inbox mail so it
  # leaves the inbox folder — and therefore Skim and the feed, which both read
  # inbox-folder mail only. Future mail is auto-archived at ingest
  # (EmailProcessJob). Scoped to the acting user's readable accounts.
  class ApplyBlock
    def self.call(contact)
      return { archived_count: 0 } unless contact

      ids = EmailMessage.accessible_to(Current.user)
                        .where(contact_id: contact.id)
                        .pluck(:id)
      return { archived_count: 0 } if ids.empty?

      Tools::BulkArchive.call("email_ids" => ids)
    end
  end
end
