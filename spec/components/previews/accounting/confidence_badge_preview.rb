# frozen_string_literal: true

module Accounting
  class ConfidenceBadgePreview < Lookbook::Preview
    # Strong match (≥ 80%) — green.
    def strong
      render(Campbooks::Accounting::ConfidenceBadge.new(match: stub_match(0.92, amount: "exact", days: 0)))
    end

    # Possible match (60–79%) — amber.
    def possible
      render(Campbooks::Accounting::ConfidenceBadge.new(match: stub_match(0.72, amount: "close", days: 3)))
    end

    # Weak match (< 60%) — neutral.
    def weak
      render(Campbooks::Accounting::ConfidenceBadge.new(match: stub_match(0.48, days: 14)))
    end

    # Strong match with cross-reconciliation warning.
    def with_cross_reconciliation_warning
      render(Campbooks::Accounting::ConfidenceBadge.new(
               match: stub_match(0.91, amount: "exact", days: 1,
                                 cross_reconciliation: true)))
    end

    # Strong match with AI reason text.
    def with_ai_reason
      render(Campbooks::Accounting::ConfidenceBadge.new(
               match: stub_match(0.89, amount: "exact", days: 2,
                                 ai_reason: "Description mentions Acme Corp matching the counterparty.")))
    end

    # Non-expandable (used inline in row cells).
    def not_expandable
      render(Campbooks::Accounting::ConfidenceBadge.new(
               match:      stub_match(0.85, amount: "exact", days: 1),
               expandable: false))
    end

    private

    def stub_match(confidence, amount: nil, days: nil, name_sim: nil,
                   cross_reconciliation: false, ai_reason: nil)
      reasons = {}
      reasons["amount"]                    = amount if amount
      reasons["date_delta_days"]           = days unless days.nil?
      reasons["name_similarity"]           = name_sim if name_sim
      reasons["cross_reconciliation_warning"] = true if cross_reconciliation
      reasons["ai_reason"]                 = ai_reason if ai_reason

      TransactionMatch.new(
        status:        :suggested,
        matched_by:    :heuristic,
        confidence:    confidence,
        match_reasons: reasons
      )
    end
  end
end
