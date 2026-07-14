# frozen_string_literal: true

module Emails
  # One-shot repair for messages synced from Zoho before Zoho::MailClient
  # decoded the HTML-entity-escaped metadata its list endpoints return
  # ("&lt;user@example.com&gt;" → "<user@example.com>"). Rows stored escaped broke
  # reply-all self-exclusion (the user got added as a recipient of their own
  # reply), address display, and sender matching.
  #
  # Enqueued once by a migration; runs as ONE serial job (never a per-message
  # fan-out, which would flood the queue and starve user jobs). Effectively
  # idempotent: decoded rows no longer match the entity pattern. update_columns
  # keeps callbacks out of the sweep — the search index catches up on its next
  # reindex.
  class ZohoEscapedFieldsBackfillJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3

    # Matches the entities Zoho's HTML-escaping emits (named + numeric). Same
    # pattern is used as a Postgres regex, so keep it to shared syntax.
    ENTITY = /&(lt|gt|amp|quot|apos|#[0-9]+);/
    FIELDS = %w[from_address to_address cc_address subject summary].freeze

    def perform
      decoded = 0
      scope.find_each do |message|
        updates = FIELDS.each_with_object({}) do |field, acc|
          value = message[field]
          acc[field] = CGI.unescapeHTML(value) if value.is_a?(String) && value.match?(ENTITY)
        end
        next if updates.empty?

        message.update_columns(updates)
        decoded += 1
      end
      Rails.logger.info("[Emails::ZohoEscapedFieldsBackfillJob] decoded #{decoded} messages")
    end

    private

    # Only Zoho-synced rows: Gmail/Graph store raw header values, where an
    # entity is legitimate literal text that must not be decoded.
    def scope
      EmailMessage
        .joins(:email_account)
        .where(email_accounts: { provider: :zoho })
        .where(FIELDS.map { |f| "#{f} ~ :entity" }.join(" OR "), entity: ENTITY.source)
    end
  end
end
