# frozen_string_literal: true

module Emails
  # Learns each user's Skim habits from their own past decisions and, when a clear
  # majority emerges, suggests the same action on the next similar card (surfaced as
  # Scout). No LLM and no rules table — the recorded SkimDecisions ARE the memory,
  # the email twin of Documents::ClassificationMemory.
  #
  # A cluster's signature resolves most-specific-first: the exact sender Contact
  # beats the sender domain beats the broad category — who sent it is a stronger
  # prior than which bucket it landed in (mirrors "sender beats filename" there). A
  # signal only counts with at least MIN_EXAMPLES cards and a dominant action holding
  # at least MIN_SHARE of them; recent only (WINDOW), since habits drift.
  #
  # Built once per deck and reused across every card: it preloads the user's recent
  # decisions in a single query and tallies them in memory, so a deck of dozens of
  # cards costs one round-trip, not one query per card.
  class SkimActionMemory
    MIN_EXAMPLES = 3
    MIN_SHARE = 0.6
    WINDOW = 90.days

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    # The strongest learned suggestion for a cluster, or nil. Returns a plain Hash
    # ({ action:, source:, count:, total: }) so SkimBuilder stays free of ActiveRecord.
    def suggestion_for(contact_id: nil, sender_domain: nil, category: nil)
      by(:contact, contact_id) ||
        by(:domain, normalize(sender_domain)) ||
        by(:category, category&.to_s.presence)
    end

    private

    def by(source, key)
      return nil if key.nil? || key == ""

      consensus(tallies.dig(source, key), source)
    end

    # Dominant action under a key, or nil when there aren't enough examples or no
    # single action holds the majority.
    def consensus(counts, source)
      return nil if counts.nil? || counts.empty?

      total = counts.values.sum
      return nil if total < MIN_EXAMPLES

      action, count = counts.max_by { |_, n| n }
      return nil if count.to_f / total < MIN_SHARE

      { action: action, source: source, count: count, total: total }
    end

    # Preload every recent decision once and bucket it into per-key action tallies
    # for all three signatures, so each card's lookup is a hash hit.
    def tallies
      @tallies ||= begin
        acc = { contact: {}, domain: {}, category: {} }
        rows = SkimDecision
          .where(user_id: @user.id, action: SkimDecision::ACTIONS)
          .where(created_at: (@now - WINDOW)..)
          .pluck(:contact_id, :sender_domain, :category, :action)

        rows.each do |contact_id, domain, category, action|
          bump(acc[:contact], contact_id, action)
          bump(acc[:domain], normalize(domain), action)
          bump(acc[:category], category.presence, action)
        end
        acc
      end
    end

    def bump(bucket, key, action)
      return if key.nil? || key == ""

      (bucket[key] ||= Hash.new(0))[action] += 1
    end

    def normalize(domain) = domain&.downcase.presence
  end
end
