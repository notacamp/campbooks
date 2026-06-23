# frozen_string_literal: true

require "digest"

module Emails
  # Upgrades multi-email Skim cluster cards with an AI "what is this about" summary,
  # off the request path. For each cluster it derives a stable digest from the member
  # email ids and either:
  #   - cache hit  → overwrites the card's templated summary with the cached sentence
  #   - cache miss → leaves the fallback and enqueues Emails::SkimSummaryJob to
  #                  generate it (deduped, so a deck of N cold cards enqueues each once)
  #
  # It also stamps card[:summary_digest] so the card carries the DOM id the job's
  # live-swap broadcast targets. Single-email cards are skipped — they already show
  # their own ai_summary (set in SkimBuilder). Applied in SkimController#show only;
  # the tray broadcaster builds the same rings but never renders summaries.
  class SkimSummaries
    CACHE_NS = "skim_cluster_summary:v1"
    CACHE_TTL = 7.days
    PENDING_TTL = 5.minutes

    def self.cache_key(digest) = "#{CACHE_NS}:#{digest}"
    def self.pending_key(digest) = "#{CACHE_NS}:pending:#{digest}"

    # A cluster is identified by its exact member set: if mail is added or cleared the
    # id-set changes, the key changes, and the summary regenerates against the new set.
    def self.digest_for(email_ids)
      Digest::SHA1.hexdigest(Array(email_ids).map(&:to_s).sort.join(","))[0, 16]
    end

    def initialize(user)
      @user = user
    end

    def apply!(rings)
      rings.each { |ring| ring[:clusters].each { |card| apply_card(card) } }
      rings
    end

    private

    def apply_card(card)
      return if card[:count].to_i < 2

      digest = self.class.digest_for(card[:email_ids])
      card[:summary_digest] = digest

      cached = Rails.cache.read(self.class.cache_key(digest))
      if cached.present?
        card[:summary] = cached
      else
        enqueue(card[:email_ids], digest)
      end
    rescue => e
      Rails.logger.warn("[Emails::SkimSummaries] #{e.class}: #{e.message}")
    end

    # Enqueue once per cold cluster per window: the pending marker is written only if
    # absent (unless_exist), so re-opening Skim while a job is in flight doesn't pile
    # up duplicate generations.
    def enqueue(email_ids, digest)
      return unless Rails.cache.write(self.class.pending_key(digest), true, expires_in: PENDING_TTL, unless_exist: true)

      Emails::SkimSummaryJob.perform_later(@user.id, Array(email_ids).map(&:to_s), digest)
    end
  end
end
