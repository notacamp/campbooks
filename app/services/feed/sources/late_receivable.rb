# frozen_string_literal: true

module Feed
  module Sources
    # A revenue invoice that has no bank line on a reconciled statement.
    # A document is only a candidate once Money::Evidence says :missing for it.
    # Until then, Money says nothing about it.
    class LateReceivable < Feed::Source
      include Feed::Sources::MoneyUsual

      def self.key = "late_receivable"

      def candidates
        evidence = Money::Evidence.for(workspace)
        late_revenue_invoices.filter_map do |doc|
          next unless evidence.status_for(doc) == :missing

          anchor = evidence.anchor_for(doc)
          next unless anchor

          days_since = (now.to_date - anchor).to_i
          ratio      = amount_ratio_for(doc)
          stmt_label = evidence.label_for(evidence.statement_for(doc)) if evidence.statement_for(doc)

          {
            subject:    doc,
            dedupe_key: "late_receivable:#{doc.id}",
            sort_at:    anchor.in_time_zone,
            score:      score_for(days_since, ratio),
            attention:  true,
            data:       {
              "anchor_date"     => anchor.iso8601,
              "days_since"      => days_since,
              "statement_label" => stmt_label,
              "amount_cents"    => doc.amount_cents,
              "currency"        => doc.currency,
              "amount_ratio"    => ratio
            }
          }
        end
      end

      def still_valid?(_item, doc)
        return false if doc.nil?

        evidence = Money::Evidence.for(workspace)
        !doc.settled? && !doc.review_rejected? &&
          doc.amount_cents.present? &&
          evidence.status_for(doc) == :missing
      end

      # The overdue-ness sets urgency: a high band that ramps with lateness, capped
      # so it never crowds out a genuine emergency. An unusually large amount boosts
      # by 8.
      def score_for(days_since, ratio = nil)
        base = [ 86 + (days_since / 3), 97 ].min
        ratio && ratio >= 2.0 ? [ base + 8, 97 ].min : base
      end

      private

      def workspace
        @workspace ||= user.try(:workspace) ||
                       Workspace.find(user.workspace_id)
      end

      def late_revenue_invoices
        Document.accessible_to(user)
                .where(workspace_id: user.workspace_id)
                .revenue_invoice
                .where(settled_at: nil)
                .reject { |doc| doc.review_rejected? || !doc.ai_completed? }
      end
    end
  end
end
