# frozen_string_literal: true

module Contacts
  # Decides whether a sender is worth an AI contact profile — the WHO question,
  # replacing the old "has this address mailed 5+ times" volume threshold. A
  # relationship's value is about who the sender is, not how often they've
  # written: an accountant's first invoice deserves a profile; a no-reply build
  # bot's thousandth never does.
  #
  # A cheap, LLM-free deny-list over signals triage has ALREADY computed on the
  # message by the time contact identification runs (the category + the RFC
  # bulk/automated headers). Mirrors Tasks::ExtractionGate / Reminders::ExtractionGate
  # — the same "is this machine traffic?" screen they use to avoid spending a
  # model call on noise.
  #
  # Keying on the triage `category` (not the raw List-Unsubscribe header) is
  # deliberate: Emails::Categorizer already files an invoice-with-unsubscribe-footer
  # as :updates but a pure newsletter as :promotions, so the nuance we want falls
  # out for free — a real vendor whose transactional mail happens to carry a list
  # header is still profiled.
  class AnalysisGate
    # Triage buckets that are machine/bulk by definition. `updates` is
    # deliberately NOT here: transactional mail (invoices, receipts, shipping) is
    # very often a real vendor relationship — an accountant's "please pay the
    # invoice" — exactly the contact we most want profiled. Mirrors
    # Tasks::ExtractionGate::MACHINE_CATEGORIES.
    MACHINE_CATEGORIES = %w[notifications promotions social].freeze

    def self.analyze?(email) = new(email).analyze?

    def initialize(email)
      @email = email
    end

    def analyze?
      return false if junk?
      return false if MACHINE_CATEGORIES.include?(@email.try(:category).to_s)
      return false if Emails::Categorizer.machine_sender?(@email)

      true
    end

    private

    # Precedence: junk is the one bulk flavour dropped outright; bulk/list still
    # pass (a real vendor can ride list-flavoured transactional mail). Mirrors the
    # sibling extraction gates.
    def junk? = @email.try(:header_precedence).to_s.strip.downcase == "junk"
  end
end
