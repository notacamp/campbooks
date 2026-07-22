module Commitments
  # Shared dedup-key logic used by both Reminders::Builder and Tasks::Builder to
  # normalise a commitment title before comparing it across sources or kinds.
  # Extracted here so the two builders stay in sync without duplicating the regexp.
  module TitleKey
    module_function

    # Trailing date/time noise the model tends to append to a title ("... on 2026-12-20",
    # "... de 20/12/2026", "... at 16:10"), plus a directly-preceding connector word. A
    # connector alone is never stripped, so this only collapses date-suffix variants and
    # can't merge genuinely different titles.
    DEDUP_TITLE_NOISE = %r{
      \s*
      (?:\b(?:on|at|em|de|le|el)\s+)?
      (?:
        \d{4}-\d{2}-\d{2} |
        \d{1,2}[/.]\d{1,2}(?:[/.]\d{2,4})? |
        \d{1,2}[:h]\d{2}
      )
    }xi

    # De-dupe match key: the thread-subject key (Re:/Fwd: stripped, case/space folded)
    # with trailing date/time tokens removed. Two titles that differ only by an
    # appended date produce the same key; genuinely different titles do not.
    def of(title)
      Emails::SubjectNormalizer.key(title.to_s).gsub(DEDUP_TITLE_NOISE, " ").squish
    end
  end
end
