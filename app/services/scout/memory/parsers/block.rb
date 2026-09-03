# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "block noreply@x.com" / "never show noreply@x.com in the stack"
      #   -> block the contact.
      class Block
        MATCHERS = [
          /\A\s*block\s+(.+?)\.?\s*\z/i,
          /\A\s*never\s+show\s+(.+?)(?:\s+in\s+the\s+(?:stack|inbox))?\.?\s*\z/i
        ].freeze

        def self.call(text)
          MATCHERS.each do |matcher|
            match = matcher.match(text.to_s)
            next unless match

            who = Shared.clean(match[1])
            return { kind: :block, contact: who } if who.present?
          end
          nil
        end
      end
    end
  end
end
