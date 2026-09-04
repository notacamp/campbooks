# frozen_string_literal: true

module Feed
  module Sources
    # A reply you owe to an established correspondent — pure data, no AI required.
    # "AI as vetter, never gatekeeper": the AI may have never read these messages
    # (no ai_action_prompt), but the human signal is unambiguous — they wrote,
    # you didn't answer, and you know this person. One card per thread.
    #
    # Distinct from ReplyReminder (which requires Scout's action prompt), this
    # source claims what the AI never flagged: unanswered mail from someone you
    # already have a relationship with, three-plus days old. A first message from
    # a stranger is NOT a reply owed — that is "New" under Recent on the People
    # list.
    #
    # Registered after ReplyReminder so an AI-flagged reply keeps its richer
    # framing; this source picks up the rest.
    class ReplyOwed < Feed::Source
      AGED_DAYS = 3
      MAX_AGE = Emails::AwaitingReply::MAX_NUDGE_AGE

      def self.key = "reply_owed"

      def candidates
        collapse_by_thread(
          base_scope.find_each.filter_map { |m| candidate_for(m) }
        )
      end

      def still_valid?(item, m)
        return false if m.nil?
        return false if m.skimmed_at.present?
        return false if m.ai_todo_dismissed?

        thread = m.email_thread
        return false if thread.nil? || thread.holds_last_word?

        in_inbox?(m)
      end

      private

      # One candidate per inbound message that satisfies all gates.
      def candidate_for(m)
        contact = m.contact
        return nil unless contact&.kind_person? && contact.sender_kind_source.present?
        return nil if Contacts::SenderKind.broadcast?(m, provider_hints: false)
        return nil if copied_only?(m)
        thread = m.email_thread
        return nil unless thread && m.received_at && thread.last_inbound_at
        return nil if m.received_at < thread.last_inbound_at # an older message they followed up on; the newest one is the ask
        return nil unless established_relationship?(m, contact)

        age = age_days(m.received_at)
        {
          subject: m,
          dedupe_key: "reply_owed:#{m.id}",
          sort_at: m.received_at,
          score: ramp(age.to_f, from: AGED_DAYS, to: 14, at_from: 30, at_to: 70),
          attention: age >= 7,
          data: { "reason" => "no_reply", "since" => m.received_at.iso8601, "age_days" => age }
        }
      end

      # The base scope: inbound messages, admitted, in inbox, not skimmed,
      # not dismissed, with a thread, aged but not stale.
      def base_scope
        in_inbox(admitted(
          EmailMessage.accessible_to(user)
                      .where(skimmed_at: nil, ai_todo_dismissed: false)
                      .where.not(email_thread_id: nil)
                      .not_answered_by_owner
                      .where("received_at <= ?", now - AGED_DAYS.days)
                      .where("received_at >= ?", now - MAX_AGE)
                      .where.not(contact_id: nil)
                      .includes(:email_thread, contact: :person)
                      .select(:id, :received_at, :email_thread_id, :contact_id,
                              :from_address, :to_address, :cc_address,
                              :subject, :email_account_id, :provider_folder_id,
                              :header_list_unsubscribe, :header_precedence,
                              :header_auto_submitted, :category, :skimmed_at,
                              :ai_todo_dismissed)
        ))
      end

      # An established relationship: the thread has an outbound from you, or the
      # contact is starred/allowed, or a VIP relationship label.
      def established_relationship?(m, contact)
        return true if contact.starred? || contact.allowed?
        return true if VIP_RELATIONSHIPS.include?(contact.person&.relationship_type.to_s)

        thread = m.email_thread
        thread&.last_outbound_at.present?
      end

      VIP_RELATIONSHIPS = People::Priority::VIP_RELATIONSHIPS

      # You are in Cc and not in To. Mirrors People::Standing#copied_only?.
      def copied_only?(message)
        owner = owner_address_for(message.email_account_id)
        return false if owner.blank?

        message.cc_address.to_s.downcase.include?(owner) &&
          !message.to_address.to_s.downcase.include?(owner)
      end

      def owner_address_for(account_id)
        @owner_addresses ||= feed_accounts.to_h { |a| [ a.id, a.email_address.to_s.downcase ] }
        @owner_addresses[account_id]
      end

      def age_days(received_at)
        return 0 if received_at.blank?

        [ ((now - received_at) / 1.day).floor, 0 ].max
      end
    end
  end
end
