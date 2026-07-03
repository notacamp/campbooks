module Learning
  module Sources
    # A Source answers one question for the engine: for a given signal tier,
    # what's the tally of human-chosen labels? Concrete sources decide WHERE the
    # verdicts live — a domain's own table (documents.review_status,
    # reminders.status) or the shared learning_decisions table — and HOW the
    # per-tier lookup key is derived.
    #
    # Two flavours share this interface:
    #   * Targeted — one query per tier, for a single subject the source is built
    #     around (e.g. Sources::Documents). `tally_for` runs a live query and the
    #     cascade short-circuits, so an expensive later tier is skipped once an
    #     earlier one wins.
    #   * Bulk — one preload buckets the whole corpus, then O(1) per-tier lookups
    #     against caller-supplied keys passed as `context` (e.g. Sources::Decisions).
    #
    # A tier with nothing returns nil so the engine skips it. The cascade is
    # ordered most-specific-first; the engine stops at the first tier that
    # reaches consensus.
    class Base
      # Ordered signal tiers, most specific first, e.g. %i[sender filename].
      def signal_cascade
        raise NotImplementedError, "#{self.class} must implement #signal_cascade"
      end

      # → { label => count } for this tier, or nil when the tier is empty.
      # `context` carries caller-supplied lookup keys for bulk sources; targeted
      # sources ignore it (they're built around their subject).
      def tally_for(_signal, **_context)
        raise NotImplementedError, "#{self.class} must implement #tally_for"
      end

      # Consensus thresholds — a tier only speaks with enough examples and a
      # clear enough majority. Same defaults both existing loops used; override
      # per source if a domain needs to be stricter/looser.
      def min_examples = 3

      def min_share = 0.6
    end
  end
end
