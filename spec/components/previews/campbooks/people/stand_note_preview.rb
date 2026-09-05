# frozen_string_literal: true

module Campbooks
  module People
    class StandNotePreview < Lookbook::Preview
      FIXED_ID = "00000000-0000-0000-0000-000000000001"

      def reply_ask
        render Campbooks::People::StandNote.new(
          standing: result(verb: :reply, detail: "the signed NDA", detail_kind: :ask_ai,
                           subject: "Partnership agreement", wait_days: 3, feed_item_id: "f1",
                           email_message_id: "m1"),
          counterpart: counterpart("Sofia"),
          can_send: true,
          reply_target: fake_message("m1")
        )
      end

      def reply_quote
        render Campbooks::People::StandNote.new(
          standing: result(verb: :reply, detail: "Could you confirm the delivery date?", detail_kind: :ask_quote,
                           subject: "Order #5872", wait_days: 1, feed_item_id: "f2",
                           email_message_id: "m2"),
          counterpart: counterpart("Rui"),
          can_send: true,
          reply_target: fake_message("m2")
        )
      end

      def reply_plain_with_draft
        render Campbooks::People::StandNote.new(
          standing: result(verb: :reply, detail: nil, detail_kind: nil,
                           subject: "Q3 project update", wait_days: 2,
                           feed_item_id: "f3", email_message_id: "m3"),
          counterpart: counterpart("Marco"),
          can_send: true,
          reply_target: fake_message("m3"),
          draft_present: true
        )
      end

      def nudge
        render Campbooks::People::StandNote.new(
          standing: result(verb: :nudge, detail: "the final invoice", detail_kind: :reason,
                           subject: "Website redesign", wait_days: 7, feed_item_id: "f4"),
          counterpart: counterpart("Inês"),
          can_send: true,
          reply_target: fake_message("m4")
        )
      end

      def decide
        render Campbooks::People::StandNote.new(
          standing: result(verb: :decide, detail: "Please approve the budget before Friday.",
                           detail_kind: :prompt, subject: "2026 budget", wait_days: 4,
                           feed_item_id: "f5"),
          counterpart: counterpart("Miguel")
        )
      end

      def pay
        render Campbooks::People::StandNote.new(
          standing: result(verb: :pay, detail_kind: :money,
                           money: { "amount_cents" => 85000, "currency" => "EUR",
                                    "due_date" => "2026-07-15", "days_late" => 50,
                                    "reference" => "INV-2026-042" },
                           subject: "INV-2026-042", wait_days: 50, feed_item_id: "f6"),
          counterpart: org_counterpart("Cloudhost")
        )
      end

      def you_wrote_last
        render Campbooks::People::StandNote.new(
          standing: result(detail: "2026-08-28", detail_kind: :you_wrote_last),
          counterpart: counterpart("Beatriz")
        )
      end

      def fresh_ask
        render Campbooks::People::StandNote.new(
          standing: result(detail: "a copy of the tax certificate", detail_kind: :ask_ai),
          counterpart: counterpart("Carlos"),
          can_send: true,
          reply_target: fake_message("m8")
        )
      end

      def nothing
        render Campbooks::People::StandNote.new(
          standing: People::Standing::Result.none,
          counterpart: counterpart("Unknown")
        )
      end

      private

      Counterpart = Struct.new(:id, :display_name, :name, keyword_init: true)

      def counterpart(first_name)
        Counterpart.new(id: FIXED_ID, display_name: "#{first_name} Example", name: "#{first_name} Example")
      end

      def org_counterpart(name)
        Counterpart.new(id: FIXED_ID, display_name: name, name: name)
      end

      def result(verb: nil, detail: nil, detail_kind: nil, money: nil, needs_you: false,
                 subject: nil, wait_days: 0, feed_item_id: nil, email_message_id: nil)
        People::Standing::Result.new(
          verb: verb, detail: detail, detail_kind: detail_kind, money: money,
          needs_you: needs_you, thread_id: nil, overdue_days: wait_days,
          kind: needs_you ? :attention : :last_exchange,
          subject: subject, wait_days: wait_days,
          feed_item_id: feed_item_id, email_message_id: email_message_id
        )
      end

      FakeMessage = Struct.new(:id, :received_at, :ai_provenance, keyword_init: true) do
        def respond_to?(method, *)
          return true if method == :ai_provenance
          super
        end

        # Route helpers build the tool URL from the message id.
        def to_param = id
      end

      def fake_message(id)
        FakeMessage.new(id: id, received_at: 2.days.ago, ai_provenance: nil)
      end
    end
  end
end
