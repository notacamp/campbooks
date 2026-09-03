# frozen_string_literal: true

module Contacts
  # Which STREAM a service's mail belongs to — the kind label shown on the
  # organization page ("Streams from Cloudhost") and used to bucket a service's
  # messages. Derived from the service contact's recent mail: an invoice/receipt
  # attachment or transactional traffic reads as billing; otherwise the message
  # category majority maps to newsletters / social / updates / notifications.
  #
  # Cached on `contacts.stream_kind`, set alongside `sender_kind` by
  # Contacts::SenderKind. Only meaningful for a service contact (a person's is nil).
  module StreamKind
    KINDS = %w[billing notifications newsletters updates social].freeze

    # The `document_type` enum values that make a service's mail "billing".
    BILLING_DOC_TYPES = %w[expense_invoice revenue_invoice receipt credit_note].freeze

    SAMPLE = 10

    class << self
      # The stream kind for a service contact, or nil when it has no mail to judge.
      def classify(contact)
        messages = recent_messages(contact)
        return nil if messages.empty?

        return "billing" if billing?(contact, messages)

        case majority_category(messages)
        when "promotions" then "newsletters"
        when "social"     then "social"
        when "updates"    then "updates"
        else                   "notifications"
        end
      end

      private

      def recent_messages(contact)
        contact.email_messages.order(received_at: :desc).limit(SAMPLE).to_a
      end

      # Billing when the sender's mail carries invoice/receipt documents, or its
      # messages are transactional (an :updates message with a transactional
      # subject — orders, invoices, receipts, shipping).
      def billing?(contact, messages)
        return true if contact.related_documents.where(document_type: BILLING_DOC_TYPES).exists?

        messages.any? do |msg|
          msg.category.to_s == "updates" && msg.subject.to_s.match?(Emails::Categorizer::TRANSACTIONAL_SUBJECT)
        end
      end

      def majority_category(messages)
        counts = messages.each_with_object(Hash.new(0)) { |m, h| h[m.category.to_s] += 1 }
        counts.max_by { |_cat, n| n }&.first
      end
    end
  end
end
