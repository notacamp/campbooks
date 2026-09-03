# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "treat GitHub notifications as a stream" / "make X a stream"
      #   -> a sender InboxGroupRule feeding a group named after the phrase.
      class Stream
        MATCHER = /\A\s*(?:treat|make)\s+(.+?)\s+(?:as\s+)?a?\s*stream\.?\s*\z/i

        def self.call(text)
          match = MATCHER.match(text.to_s)
          return nil unless match

          phrase = Shared.clean(match[1])
          return nil if phrase.blank?

          { kind: :stream, value: Shared.sender_token(phrase), group: phrase }
        end
      end
    end
  end
end
