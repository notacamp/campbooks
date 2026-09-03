# frozen_string_literal: true

module Contacts
  # Classifies a Contact as a person or a service — the Rethink "who is talking"
  # axis. A service is a sender whose mail is machine/bulk: newsletters, receipts,
  # notifications, alerts. It reuses the human-or-automated verdict Skim already
  # computes for every message (Emails::Categorizer) over the sender's recent
  # mail, so the People place needs no new AI.
  #
  # Verdict per the sender's last N messages: a message reads "service-flavoured"
  # when it is an automated sender, carries bulk/list/auto-submitted headers, or
  # is filed in a machine category (notifications/promotions/social/updates). When
  # the majority of the sample is service-flavoured the contact is a service; else
  # a person. A `taught` source (the user corrected it by hand) is never overridden.
  #
  # A service verdict also caches its Contacts::StreamKind and, lazily, links the
  # service to its domain's organization (Organizations::FromDomain).
  module SenderKind
    SAMPLE = 10

    # Triage categories that are machine/bulk by definition (the People axis is
    # coarser than Contacts::AnalysisGate: `updates` counts here, so a receipts
    # service reads as a service even though we'd still profile a real vendor).
    SERVICE_CATEGORIES = %w[notifications promotions social updates].freeze

    class << self
      # Classify and persist the contact's sender_kind (+ stream_kind for a
      # service). No-op when the user taught it or when there's no mail to judge.
      # Returns the resulting kind symbol (:person / :service), or nil when skipped.
      def classify(contact)
        return nil if contact.nil? || contact.sender_kind_taught?

        messages = recent_messages(contact)
        return nil if messages.empty?

        kind = service?(messages) ? :service : :person
        persist(contact, kind)
        kind
      end

      # Pure verdict for a sampled message set (no persistence) — the unit under test.
      def service?(messages)
        return false if messages.empty?

        service_flavoured = messages.count { |msg| service_message?(msg) }
        service_flavoured * 2 > messages.size
      end

      # One message reads as service traffic: automated sender, bulk/list/auto-
      # submitted headers, or a machine triage category.
      def service_message?(msg)
        Emails::Categorizer.machine_sender?(msg) ||
          Emails::Categorizer.new(msg).call.noise? ||
          msg.header_list_unsubscribe.to_s.strip.present? ||
          %w[bulk list junk].include?(msg.header_precedence.to_s.strip.downcase) ||
          auto_submitted?(msg) ||
          SERVICE_CATEGORIES.include?(msg.category.to_s)
      end

      private

      def recent_messages(contact)
        contact.email_messages.order(received_at: :desc).limit(SAMPLE).to_a
      end

      def auto_submitted?(msg)
        value = msg.header_auto_submitted.to_s.strip.downcase
        value.present? && value != "no"
      end

      def persist(contact, kind)
        stream_kind = kind == :service ? Contacts::StreamKind.classify(contact) : nil
        contact.update_columns(
          sender_kind: Contact.sender_kinds.fetch(kind.to_s),
          sender_kind_source: "heuristic",
          stream_kind: stream_kind
        )
        # Reload the enum so downstream `kind_service?` reads true before linking.
        contact.sender_kind = kind.to_s
        Organizations::FromDomain.link(contact) if kind == :service
      end
    end
  end
end
