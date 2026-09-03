# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "mail from sofia@x.com is priority" / "make sofia@x.com priority"
      #   -> star the contact.
      class Priority
        MATCHERS = [
          /\A\s*(?:mail\s+from\s+)?(.+?)\s+is\s+(?:a\s+)?priority\.?\s*\z/i,
          /\A\s*make\s+(?:mail\s+from\s+)?(.+?)\s+(?:a\s+)?priority\.?\s*\z/i
        ].freeze

        def self.call(text)
          MATCHERS.each do |matcher|
            match = matcher.match(text.to_s)
            next unless match

            who = Shared.clean(match[1])
            return { kind: :priority, contact: who } if who.present?
          end
          nil
        end
      end
    end
  end
end
