# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "tag mail from @edp.pt as Utilities" -> an EmailRule that tags mail from a
      # sender.
      class TagRule
        MATCHER = /\A\s*tag\s+mail\s+from\s+(.+?)\s+as\s+(.+?)\.?\s*\z/i

        def self.call(text)
          match = MATCHER.match(text.to_s)
          return nil unless match

          from = Shared.clean(match[1])
          tag = Shared.clean(match[2]).delete_prefix("#")
          return nil if from.blank? || tag.blank?

          { kind: :tag, from: from, tag: tag }
        end
      end
    end
  end
end
