# frozen_string_literal: true

module Feed
  module Sources
    # Chasing is a card. A late receivable — a revenue invoice you sent, past its due
    # date and not yet settled by the bank — becomes an attention card on Now, with
    # the reminder already drafted. It rides on the same Money substrate as the Money
    # page (Document#direction / #settled? / due_date), so the two never disagree.
    class LateReceivable < Feed::Source
      include Feed::Sources::MoneyUsual

      def self.key = "late_receivable"

      def candidates
        late_revenue_invoices.filter_map do |doc|
          due = safe_due(doc)
          next unless due && doc.amount_cents.present? && due < now.to_date

          days_late = (now.to_date - due).to_i
          ratio = amount_ratio_for(doc)
          {
            subject: doc,
            dedupe_key: "late_receivable:#{doc.id}",
            sort_at: due.in_time_zone, # a stable anchor (the due date), never `now`
            score: score_for(days_late, ratio),
            attention: true,
            data: { "due_date" => due.iso8601, "days_late" => days_late,
                    "amount_cents" => doc.amount_cents, "currency" => doc.currency,
                    "amount_ratio" => ratio }
          }
        end
      end

      def still_valid?(_item, doc)
        return false if doc.nil?

        due = safe_due(doc)
        doc.direction == :receivable && !doc.settled? && !doc.review_rejected? &&
          doc.amount_cents.present? && due.present? && due < now.to_date
      end

      private

      def late_revenue_invoices
        Document.accessible_to(user)
                .where(workspace_id: user.workspace_id)
                .revenue_invoice
                .where(settled_at: nil)
                .reject { |doc| doc.review_rejected? || !doc.ai_completed? }
      end

      # The overdue-ness sets urgency: a high band that ramps with lateness, capped
      # so it never crowds out a genuine emergency. An unusually large amount boosts
      # the score by 8.
      def score_for(days_late, ratio = nil)
        base = [ 86 + (days_late / 3), 97 ].min
        ratio && ratio >= 2.0 ? [ base + 8, 97 ].min : base
      end

      def safe_due(doc)
        doc.due_date&.to_date
      rescue StandardError
        nil
      end
    end
  end
end
