module Tasks
  # Cheap pre-filter deciding whether an email is worth an LLM task-extraction call.
  # Most mail contains no action for the reader, so we skip those without spending a
  # model call. Tuned for action-request language (distinct from Reminders'
  # date-centric keywords). Leans permissive — the LLM + confidence floor reject the
  # FYI mail that slips through.
  class ExtractionGate
    KEYWORDS = /\b(?:please|kindly|could\s+you|can\s+you|would\s+you|need\s+you\s+to|
                  action\s+required|to-?do|follow[\s-]?up|let\s+me\s+know|get\s+back\s+to\s+(?:me|us)|
                  send\s+(?:me|us|over|back)|forward\s+(?:me|us)|review|approv\w*|sign\s+off|signature|
                  confirm|complete|fill\s+out|submit|provide|respond|reply|reach\s+out|prepare|
                  required|requested|awaiting\s+your|your\s+(?:input|feedback|response|approval|sign))\b/xi

    def self.email_allows?(email)
      new.email_allows?(email)
    end

    def email_allows?(email)
      return false if junk?(email)

      text = [ email.subject, email.try(:ai_summary), strip(email.body) ].compact.join(" ")
      text.match?(KEYWORDS)
    end

    private

    # Only clear junk; "bulk"/"list" still pass (an action request can ride on a
    # notification-style message).
    def junk?(email)
      email.try(:header_precedence).to_s.strip.downcase == "junk"
    end

    def strip(body)
      ActionController::Base.helpers.strip_tags(body.to_s)
    end
  end
end
