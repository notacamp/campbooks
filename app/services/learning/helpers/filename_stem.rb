module Learning
  module Helpers
    # Normalize a filename to a token-set "stem": downcase, drop the extension,
    # strip digits (dates, invoice/policy numbers) and separators, drop short
    # tokens, then sort+dedupe so order and numbering don't matter. e.g.
    # "Fatura_EDP_2026-01.pdf" and "fatura-edp-2026-02.pdf" both → "edp fatura".
    #
    # Extracted verbatim from Documents::ClassificationMemory so any domain that
    # keys on a filename shares the exact same normalization.
    module FilenameStem
      module_function

      def call(name)
        base = name.to_s.downcase
        base = base.sub(/\.[a-z0-9]{1,5}\z/, "")
        base = base.gsub(/[0-9]+/, " ").gsub(/[^a-z]+/, " ")
        base.split.select { |t| t.length >= 3 }.uniq.sort.join(" ").presence
      end
    end
  end
end
