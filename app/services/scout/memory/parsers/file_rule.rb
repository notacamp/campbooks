# frozen_string_literal: true

module Scout
  module Memory
    module Parsers
      # "file anything from @edp.pt under Utilities" -> an EmailRule that files
      # mail from a sender into a folder.
      class FileRule
        MATCHER = /\A\s*file\s+(?:anything|everything|mail|all)?\s*from\s+(.+?)\s+(?:under|in|into|to)\s+(.+?)\.?\s*\z/i

        def self.call(text)
          match = MATCHER.match(text.to_s)
          return nil unless match

          from = Shared.clean(match[1])
          folder = Shared.clean(match[2])
          return nil if from.blank? || folder.blank?

          { kind: :file, from: from, folder: folder }
        end
      end
    end
  end
end
