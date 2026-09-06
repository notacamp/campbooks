# frozen_string_literal: true

module Feed
  module Sources
    # Shared helper for late_payable / late_receivable: computes the usual amount
    # for a document against its recurrence group (all money docs in the workspace)
    # and returns the ratio. One query, memoized per source instance.
    module MoneyUsual
      private

      # Float or nil — the doc's amount relative to the median amount in its group.
      def amount_ratio_for(doc)
        key = Money::Recurrence.group_key(doc)
        return nil unless key

        others = usual_amounts_by_key[key].to_a - [ doc.amount_cents ]
        return nil unless others.size >= 2

        usual = sorted_median(others)
        return nil if usual.to_f.zero?

        doc.amount_cents.to_f / usual
      end

      # { group_key => [amount_cents, …] } across all workspace money docs, loaded in memory.
      # Uses Money::Recurrence.group_key so the key is identical to what Recurrence uses.
      def usual_amounts_by_key
        @usual_amounts_by_key ||= begin
          Document.where(workspace_id: user.workspace_id)
                  .money_types
                  .each_with_object({}) do |doc, acc|
            key = Money::Recurrence.group_key(doc)
            next unless key && doc.amount_cents.present?

            (acc[key] ||= []) << doc.amount_cents
          end
        end
      end

      def sorted_median(array)
        sorted = array.sort
        mid = sorted.size / 2
        sorted.size.odd? ? sorted[mid].to_f : (sorted[mid - 1] + sorted[mid]) / 2.0
      end
    end
  end
end
