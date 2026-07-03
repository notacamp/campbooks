# frozen_string_literal: true

module Emails
  # Learns each user's Skim habits from their own past decisions and, when a clear
  # majority emerges, suggests the same action on the next similar card (surfaced as
  # Scout). No LLM and no rules table — the recorded decisions ARE the memory, the
  # email twin of Documents::ClassificationMemory.
  #
  # A cluster's signature resolves most-specific-first: the exact sender Contact
  # beats the sender domain beats the broad category — who sent it is a stronger
  # prior than which bucket it landed in. A signal only counts with enough cards and
  # a dominant action holding a clear majority; recent only (WINDOW), since habits
  # drift. The consensus/cascade machinery now lives in the generic Learning::
  # substrate; this is the Skim-facing adapter that names the tiers and shapes the
  # result for SkimBuilder.
  #
  # Built once per deck and reused across every card: the Learning::Sources::Decisions
  # source preloads the user's recent decisions in a single query and tallies them in
  # memory, so a deck of dozens of cards costs one round-trip, not one query per card.
  class SkimActionMemory
    WINDOW = 90.days

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    # The strongest learned suggestion for a cluster, or nil. Returns a plain Hash
    # ({ action:, source:, count:, total: }) so SkimBuilder stays free of ActiveRecord.
    def suggestion_for(contact_id: nil, sender_domain: nil, category: nil)
      raw = memory.suggestion(contact: contact_id, domain: sender_domain, category: category)
      return nil unless raw

      { action: raw.label, source: raw.source, count: raw.count, total: raw.total }
    end

    private

    def memory
      @memory ||= Learning::Memory.new(source: source)
    end

    # Skim decisions live in learning_decisions under the "email_skim" domain, scoped
    # to this user, matched most-specific-first. Domain keys are compared
    # case-insensitively and blank categories are ignored — the same normalization is
    # applied to both stored rows and lookup keys inside Learning::Sources::Decisions.
    def source
      Learning::Sources::Decisions.new(
        domain: "email_skim",
        scope: { user_id: @user.id },
        window: WINDOW,
        now: @now,
        tiers: [
          Learning::Sources::Decisions.tier(:contact, :contact_id),
          Learning::Sources::Decisions.tier(:domain, :sender_domain, normalize: ->(v) { v&.downcase.presence }),
          Learning::Sources::Decisions.tier(:category, :category, normalize: ->(v) { v.to_s.presence })
        ]
      )
    end
  end
end
