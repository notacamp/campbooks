# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # Shared helpers for the deterministic teach-sentence parsers. Each parser
      # is pure: String in, a bounded intent Hash (or nil) out. The Teacher then
      # executes the intent — parsers never touch the database.
      module Shared
        module_function

        QUOTES = "\"'\u2018\u2019\u201C\u201D"

        # Trim surrounding quotes / whitespace / a trailing period from a captured
        # fragment.
        def clean(fragment)
          fragment.to_s.strip.sub(/\.\z/, "").gsub(/\A[#{QUOTES}]+|[#{QUOTES}]+\z/, "").strip
        end

        # A sender token for a rule/contact: a full email, an @domain, or — failing
        # those — the first meaningful word, lower-cased (a bare-word ILIKE still
        # matches "github" senders).
        def sender_token(fragment)
          text = clean(fragment)
          if (email = text[/[\w.+-]+@[\w.-]+\.\w+/])
            email.downcase
          elsif (domain = text[/@[\w.-]+\.\w+/])
            domain.downcase
          else
            text.split(/\s+/).first.to_s.downcase
          end
        end

        def email?(fragment)
          clean(fragment).match?(/\A[\w.+-]+@[\w.-]+\.\w+\z/)
        end
      end
    end
  end
end
