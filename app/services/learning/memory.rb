module Learning
  # The generic consensus engine shared by every learning loop. Walks a Source's
  # ordered signal cascade and returns the first tier that reaches consensus:
  # enough examples (`min_examples`) and a dominant label holding a clear
  # majority (`min_share`). Domain-agnostic — it knows nothing about documents,
  # reminders, or skim; the Source supplies the per-tier tallies.
  #
  #   memory = Learning::Memory.new(source: Learning::Sources::Documents.new(doc))
  #   memory.suggestion                                   # targeted source → Suggestion | nil
  #   memory.suggestion(contact: id, domain: "x.com")    # bulk source → Suggestion | nil
  #
  # This replaces the duplicated consensus code that lived in both
  # Documents::ClassificationMemory and Emails::SkimActionMemory.
  class Memory
    def initialize(source:)
      @source = source
    end

    # The strongest tier that reaches consensus, or nil. `context` is forwarded
    # to the source as the per-tier lookup keys (bulk sources); targeted sources
    # ignore it. Short-circuits at the first winning tier so later (possibly
    # expensive) tiers aren't computed.
    def suggestion(**context)
      @source.signal_cascade.each do |signal|
        result = consensus(@source.tally_for(signal, **context), signal)
        return result if result
      end
      nil
    end

    private

    # { label => count } → a Learning::Suggestion when a single label clears both
    # thresholds, else nil.
    def consensus(counts, signal)
      return nil if counts.blank?

      total = counts.values.sum
      return nil if total < @source.min_examples

      label, count = counts.max_by { |_, c| c }
      return nil if count.to_f / total < @source.min_share

      Learning::Suggestion.new(label: label, source: signal, count: count, total: total)
    end
  end
end
