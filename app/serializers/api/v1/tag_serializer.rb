# frozen_string_literal: true

module Api
  module V1
    class TagSerializer
      def initialize(tag)
        @tag = tag
      end

      def as_json
        {
          id: @tag.id,
          name: @tag.name,
          color: @tag.color,
          group_name: @tag.group_name,
          source: @tag.source,
          email_account_id: @tag.email_account_id
        }
      end
    end
  end
end
