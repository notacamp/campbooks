# frozen_string_literal: true

module Campbooks
  # The cluster card's "what is this about" summary line. Extracted into its own
  # component so the SkimCard and the async Emails::SkimSummaryJob render identical
  # markup — the job live-swaps this <p>, by its digest-keyed DOM id, the moment the
  # AI summary is ready. Single-email / fallback cards render it with no id (no swap).
  class SkimSummary < Campbooks::Base
    def self.dom_id(digest) = "skim_summary_#{digest}"

    # @param text [String, nil] the summary sentence
    # @param digest [String, nil] cluster digest → DOM id for the live-swap (multi-
    #   email viewer cards only); nil renders no id
    # @param fill [Boolean] tall story-frame layout (larger text)
    def initialize(text:, digest: nil, fill: true)
      @text = text
      @digest = digest
      @fill = fill
    end

    def view_template
      return if @text.to_s.strip.empty?

      p(
        id: (@digest ? self.class.dom_id(@digest) : nil),
        class: class_names("text-muted-foreground leading-relaxed", @fill ? "mt-2 text-sm sm:text-base" : "mt-1.5 text-sm")
      ) { @text }
    end
  end
end
