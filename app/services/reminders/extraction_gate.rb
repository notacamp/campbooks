module Reminders
  # Cheap pre-filter deciding whether an email is worth an LLM reminder-extraction
  # call. The point is cost: most mail has no dated commitment, so we skip those
  # without spending a model call.
  #
  # Machine / automated senders (no-reply@, Auto-Submitted header) and explicit
  # bulk-traffic headers (List-Unsubscribe per RFC 2369; Precedence: bulk/list/junk
  # per RFC 2076) are skipped outright — they cannot carry a person-to-person
  # dated commitment. Everything that passes the sender/header check must also
  # mention a date or a reminder keyword; the LLM + confidence floor catch the
  # residual noise. Tunable — lean permissive (the product goal is "everything
  # with a date").
  class ExtractionGate
    KEYWORDS = /\b(?:due|payable|payment|invoice|bill|deliver(?:y|ed|ies)?|arriv(?:e|es|al|ing)|
                  ship(?:s|ped|ment|ping)?|tracking|dispatch(?:ed)?|renew(?:s|al|als|ing)?|
                  expir(?:e|es|y|ing|ation)|deadline|appointment|meeting|reschedul\w*|schedul(?:e|ed)|
                  reservation|booking|check\-?in|RSVP|reminder|valid\s+until|complete\s+by|respond\s+by)\b/xi

    DATE_HINT = %r{
      \b\d{4}-\d{2}-\d{2}\b |                                  # ISO 2026-07-15
      \b\d{1,2}[/.]\d{1,2}(?:[/.]\d{2,4})?\b |                 # 15/07 or 15.07.2026
      \b(?:\d{1,2}\s+)?(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?(?:\s+\d{1,4})? |  # 12 Jun / June 12
      \b(?:mon|tue|wed|thu|fri|sat|sun)[a-z]*\b |              # weekday
      \btomorrow\b | \btonight\b |
      \bnext\s+(?:week|month|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b
    }xi

    def self.email_allows?(email)
      new.email_allows?(email)
    end

    def email_allows?(email)
      return false if machine_or_bulk?(email)

      text = [ email.subject, email.try(:ai_summary), strip(email.body) ].compact.join(" ")
      relevant?(text)
    end

    def relevant?(text)
      text.match?(DATE_HINT) || text.match?(KEYWORDS)
    end

    private

    # Machine senders (Auto-Submitted, no-reply@ variants), List-Unsubscribe, and
    # Precedence: bulk/list/junk are all hard-blocked. These signals mean the email
    # is automated or bulk traffic that cannot carry a personal dated commitment.
    def machine_or_bulk?(email)
      Emails::Categorizer.machine_sender?(email) ||
        Emails::Categorizer.bulk_headers?(email)
    end

    def strip(body)
      ActionController::Base.helpers.strip_tags(body.to_s)
    end
  end
end
