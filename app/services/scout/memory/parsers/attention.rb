# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "Sofia is important" / "Sofia matters to me" / "ignore newsletters from X"
      #   -> record an attention verdict for a person.
      class Attention
        IMPORTANT_MATCHERS = [
          /\A\s*(?:mail\s+from\s+)?(.+?)\s+(?:is|are)\s+(?:very\s+)?important\.?\s*\z/i,
          /\A\s*(.+?)\s+matters?(?:\s+to\s+me)?\.?\s*\z/i,
          /\A\s*(?:always\s+)?(?:prioriti[sz]e|pay\s+attention\s+to)\s+(.+?)\.?\s*\z/i
        ].freeze

        UNIMPORTANT_MATCHERS = [
          /\A\s*(?:mail\s+from\s+)?(.+?)\s+(?:is|are)\s+not\s+important\.?\s*\z/i,
          /\A\s*(.+?)\s+(?:doesn't|does\s+not|don't|do\s+not)\s+matter\.?\s*\z/i,
          /\A\s*(?:ignore|de-?prioriti[sz]e|keep\s+(.+?)\s+out\s+of\s+the\s+way)\s*(.+?)?\.?\s*\z/i
        ].freeze

        def self.call(text)
          # Check unimportant first: "doesn't matter" / "does not matter" could be
          # greedily captured by the important `matters?` pattern otherwise.
          UNIMPORTANT_MATCHERS.each_with_index do |matcher, idx|
            match = matcher.match(text.to_s)
            next unless match
            # The third pattern (ignore/keep…out of the way) has two capture groups;
            # group 1 captures the middle name in "keep X out of the way", group 2
            # captures the name in "ignore X" / "de-prioritize X".
            who = if idx == 2
              Shared.clean(match[1].presence || match[2].to_s)
            else
              Shared.clean(match[1])
            end
            return { kind: :attention, contact: who, label: "unimportant" } if who.present?
          end

          IMPORTANT_MATCHERS.each do |matcher|
            match = matcher.match(text.to_s)
            next unless match
            who = Shared.clean(match[1])
            return { kind: :attention, contact: who, label: "important" } if who.present?
          end

          nil
        end
      end
    end
  end
end
