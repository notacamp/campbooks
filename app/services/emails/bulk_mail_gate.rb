# frozen_string_literal: true

module Emails
  # Cheap, LLM-free deny-list for the high-cost AI calls (triage embedding +
  # LLM tag-pick, search re-embedding). Mirrors Contacts::AnalysisGate but runs
  # entirely on ingest-time signals that are already persisted on the email row,
  # so it gates the triage path itself — before any category has been derived.
  #
  # Returns false (skip AI) for:
  #   • Machine / automated senders (Auto-Submitted header, no-reply@ variants
  #     — any signal Emails::Categorizer.machine_sender? detects).
  #   • Explicit bulk-traffic headers (List-Unsubscribe per RFC 2369;
  #     Precedence: bulk/list/junk per RFC 2076).
  #
  # Returns true (run AI) for everything that is plausibly person-to-person.
  # Ambiguous or missing signals let the email through — conservative by design.
  #
  # Note: the triage category (notifications/promotions/social) is NOT used here
  # because it hasn't been derived yet at the point this gate fires. The raw
  # header/sender signals catch the same traffic without a chicken-and-egg
  # dependency on the categorizer's output.
  class BulkMailGate
    def self.analyze?(email)
      new(email).analyze?
    end

    def initialize(email)
      @email = email
    end

    # Returns true when the email should receive the full AI treatment.
    def analyze?
      # An unattended / automated mailbox (no-reply@, Auto-Submitted, etc.)
      # can never carry a person-to-person conversation worth analysing.
      return false if Emails::Categorizer.machine_sender?(@email)

      # List-Unsubscribe (RFC 2369) and Precedence: bulk/list/junk (RFC 2076)
      # are canonical signals that the message is list/bulk traffic, not 1:1 mail.
      return false if Emails::Categorizer.bulk_headers?(@email)

      true
    end
  end
end
