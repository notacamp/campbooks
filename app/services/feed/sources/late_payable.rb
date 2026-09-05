# frozen_string_literal: true

module Feed
  module Sources
    # Money you owe: a past-due expense invoice that has not been settled.
    # Mirror of LateReceivable (revenue invoices you're chasing) for the payable
    # direction. Rides the same Money substrate as the Money page — the two surfaces
    # never disagree.
    class LatePayable < Feed::Source
      include Feed::Sources::MoneyUsual

      def self.key = "late_payable"

      def candidates
        late_expense_invoices.filter_map do |doc|
          due = safe_due(doc)
          next unless due && doc.amount_cents.present? && due < now.to_date

          days_late = (now.to_date - due).to_i
          ratio = amount_ratio_for(doc)
          {
            subject: doc,
            dedupe_key: "late_payable:#{doc.id}",
            sort_at: due.in_time_zone,
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
        doc.direction == :payable && !doc.settled? && !doc.review_rejected? &&
          doc.amount_cents.present? && due.present? && due < now.to_date
      end

      private

      def late_expense_invoices
        Document.accessible_to(user)
                .where(workspace_id: user.workspace_id)
                .expense_invoice
                .where(settled_at: nil)
                .reject { |doc| doc.review_rejected? || !doc.ai_completed? }
      end

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
