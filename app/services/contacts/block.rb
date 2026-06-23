module Contacts
  # Block a sender and remove their existing inbox mail. `block!` flips the flag
  # inline (so the UI updates immediately); SenderBlockArchiveJob archives their
  # already-ingested mail in the background. Future mail is auto-archived at
  # ingest (EmailProcessJob#apply_sender_rules). Mirrors the `block_sender`
  # EmailAction so blocking behaves identically from the contacts page, the
  # inbox, and the skim.
  module Block
    def self.call(contact, user: nil)
      return unless contact

      contact.block!
      SenderBlockArchiveJob.perform_later(contact.id, user&.id)
    end
  end
end
