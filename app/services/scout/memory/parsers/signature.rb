# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "sign replies with Long signature" -> make that saved signature the default.
      class Signature
        MATCHER = /\A\s*sign\s+(?:replies|mail|emails|email)\s+with\s+(?:the\s+)?(.+?)(?:\s+signature)?\.?\s*\z/i

        def self.call(text)
          match = MATCHER.match(text.to_s)
          return nil unless match

          name = Shared.clean(match[1])
          return nil if name.blank?

          { kind: :signature, name: name }
        end
      end
    end
  end
end
