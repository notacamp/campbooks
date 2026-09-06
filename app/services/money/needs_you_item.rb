# frozen_string_literal: true

class Money
  # A single item in the "Needs you" section. Built by Money::Page.
  #
  # kind: :no_invoice | :review | :partial | :nif        (lines of the newest statement)
  #       :loan_missed | :loan_changed | :loan_suggestion (the loan)
  #
  # `payload` carries what the loan rows render from: { loan:, instalment: } for
  # the alerts, a Loans::Spotter::Suggestion for a suggestion.
  NeedsYouItem = Struct.new(
    :kind, :transaction, :match, :group, :title, :meta, :actions, :payload,
    keyword_init: true
  ) do
    def loan? = kind.to_s.start_with?("loan_")
  end
end
