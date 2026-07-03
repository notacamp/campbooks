module Learning
  # Derives the (contact, sender-domain) learning signals from the source record of
  # a reminder or task. EmailMessage sources carry them directly; Document sources
  # fall back to their first linked email; anything else (or any error) yields blank
  # signals, so the caller degrades gracefully to the least-specific (type/global)
  # tier instead of failing.
  module EmailSignals
    module_function

    # → { contact_id:, sender_domain: } (either may be nil).
    def for(source)
      email =
        case source
        when EmailMessage then source
        when Document then source.email_messages.first
        end
      return blank unless email

      { contact_id: email.contact_id, sender_domain: Emails::SenderDomain.for(email.from_address) }
    rescue => e
      Rails.logger.warn("[Learning::EmailSignals] #{e.class}: #{e.message}")
      blank
    end

    def blank = { contact_id: nil, sender_domain: nil }
  end
end
