# frozen_string_literal: true

module Emails
  # Generates and caches the AI summary for one Skim cluster, then live-swaps it into
  # any open Skim viewer. Enqueued by Emails::SkimSummaries on a cache miss. Idempotent
  # and best-effort: early-returns if already cached, and a missing text model just
  # leaves the templated fallback in place.
  class SkimSummaryJob < ApplicationJob
    queue_as :default

    def perform(user_id, email_ids, digest)
      user = User.find_by(id: user_id)
      return unless user

      key = Emails::SkimSummaries.cache_key(digest)
      return if Rails.cache.exist?(key)

      # Scope to the user's readable mail (forge-proof, mirrors SkimDecisionRecorder).
      emails = EmailMessage
        .where(email_account: user.readable_email_accounts, id: email_ids)
        .order(received_at: :desc)
        .to_a
      return if emails.empty?

      Current.workspace = user.workspace
      summary = Ai::SkimClusterSummarizer.new(emails).summary
      return if summary.blank?

      Rails.cache.write(key, summary, expires_in: Emails::SkimSummaries::CACHE_TTL)
      broadcast(user, digest, summary)
    ensure
      Current.workspace = nil
    end

    private

    # Replace the card's summary <p> (by its digest-keyed DOM id) in any open viewer
    # subscribed to the user's Skim stream. Best-effort: no open viewer simply means
    # nothing matches the target, and the warm cache serves the next open.
    def broadcast(user, digest, summary)
      html = ApplicationController.render(
        Campbooks::SkimSummary.new(text: summary, digest: digest),
        layout: false
      )
      Turbo::StreamsChannel.broadcast_replace_to(
        "skim_#{user.id}",
        target: Campbooks::SkimSummary.dom_id(digest),
        html: html
      )
    rescue => e
      Rails.logger.warn("[Emails::SkimSummaryJob] broadcast failed: #{e.message}")
    end
  end
end
