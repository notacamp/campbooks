# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Honest built-in behaviour. The spec offered two candidate defaults; both
      # turned out NOT to exist in the code, so they are omitted (the honesty rule
      # from docs/messaging.md: never state a default the code does not implement):
      #   * "Documents under N% confidence go to review" — there is NO confidence
      #     threshold; every analyzed document needs review regardless of
      #     confidence (Ai::DocumentAnalyzer sets review_status: :pending always).
      #   * "Receipts are filed silently / only mentioned in the log" — no such
      #     silent-receipt handling exists anywhere.
      #
      # What IS true and worth stating: nothing is auto-filed — every document
      # Scout reads waits for your review before it is final. That single, honest
      # default is all this source emits.
      class Defaults < Base
        def entries
          [ review_entry ]
        end

        private

        def review_entry
          build(
            id: "default:review",
            facet: :filing,
            sentence: sentence("scout_memory.sources.defaults.review"),
            origin: :default,
            origin_detail: I18n.t("scout_memory.origins.default"),
            record: nil,
            form_path: nil,
            actions: []
          )
        end
      end
    end
  end
end
