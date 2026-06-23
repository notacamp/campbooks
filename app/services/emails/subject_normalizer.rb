module Emails
  # Canonicalizes an email subject for two jobs:
  #   .display — a clean, human-readable thread subject (leading reply/forward
  #              markers and [bracket] tags stripped, original case kept).
  #   .key     — a match key for grouping messages into one EmailThread: the
  #              display subject case-folded with whitespace collapsed.
  #
  # The old threader stripped only ONE leading "Re:"/"Fwd:" and was
  # case-sensitive, so "RE: FW: X", "FW: FW: X", and "Test"/"test" each spawned a
  # separate thread (one conversation → many EmailThread rows). This strips the
  # whole leading run of prefixes/tags in one pass, locale-aware, case-insensitive.
  module SubjectNormalizer
    # A leading run of one-or-more reply/forward markers and/or [bracket] tags,
    # in any order. Each marker is a known token (en/pt/es/fr/de/nordic/polish),
    # an optional [n] counter (Outlook's "Re[2]:"), then a colon (ASCII or CJK).
    # The trailing `+` consumes the entire stack ("Re: Fwd: [List] …") at once.
    PREFIX_RUN = %r{
      \A\s*
      (?:
        \[[^\]]*\]\s*                                            # a [bracket] tag
        |
        (?:re|r[ée]p|fwd?|aw|antw(?:ort)?|sv|vs|wg|tr|enc|rv|odp) # reply or forward token
        \s*(?:\[\d+\])?\s*[:：]\s*
      )+
    }ix

    module_function

    def display(subject)
      subject.to_s.sub(PREFIX_RUN, "").strip
    end

    def key(subject)
      display(subject).downcase.gsub(/\s+/, " ").strip
    end
  end
end
