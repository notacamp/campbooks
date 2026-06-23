# frozen_string_literal: true

module Api
  module V1
    # Lists the workspace's tags. Tags apply to email messages only (documents are
    # categorized by document type, not tagged) — attach/detach lives under
    # /api/v1/emails/:email_id/tags (Api::V1::EmailTagsController).
    class TagsController < BaseController
      before_action -> { doorkeeper_authorize! :"tags:read" }, only: [ :index ]

      def index
        @pagy, tags = pagy(Current.workspace.tags.by_name, limit: per_page)
        render_page(tags.map { |tag| TagSerializer.new(tag).as_json }, @pagy)
      end
    end
  end
end
