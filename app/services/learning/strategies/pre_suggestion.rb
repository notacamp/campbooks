module Learning
  module Strategies
    # One "way to train": surface the consensus label as a deterministic pre-fill
    # in the UI (no LLM). This is what Skim does — pre-suggest the action the user
    # keeps taking on similar cards. Identity today; kept as a seam so a future
    # decorator (e.g. confidence gating, or suppressing when the AI strongly
    # disagrees) can intercept here without touching call sites.
    #
    # Generalizes Emails::SkimActionMemory's suggestion surfacing.
    module PreSuggestion
      module_function

      # → the label the UI should pre-select, or nil.
      def call(suggestion)
        suggestion&.label
      end
    end
  end
end
