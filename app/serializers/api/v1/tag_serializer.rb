# frozen_string_literal: true

module Api
  module V1
    class TagSerializer
      # email_count is opt-in — the tag list endpoints pass a batched count; it is
      # omitted elsewhere (e.g. tags serialized inline on an email) to avoid an
      # N+1 of COUNT queries.
      def initialize(tag, email_count: nil)
        @tag = tag
        @email_count = email_count
      end

      def as_json
        json = {
          id: @tag.id,
          name: @tag.name,
          color: @tag.color,
          group_name: @tag.group_name,
          source: @tag.source,
          kind: @tag.kind,
          hidden: @tag.hidden,
          email_account_id: @tag.email_account_id
        }
        json[:email_count] = @email_count unless @email_count.nil?
        json
      end
    end
  end
end
