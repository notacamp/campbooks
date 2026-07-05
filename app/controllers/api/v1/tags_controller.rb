# frozen_string_literal: true

module Api
  module V1
    # Lists the workspace's tags. Tags apply to email messages only (documents are
    # categorized by document type, not tagged) — attach/detach lives under
    # /api/v1/emails/:email_id/tags (Api::V1::EmailTagsController).
    class TagsController < BaseController
      before_action -> { doorkeeper_authorize! :"tags:read" }, only: [ :index ]

      # Hidden provider labels (Gmail system statuses like INBOX / CATEGORY_*, and
      # AI-judged low-value labels) are excluded by default, mirroring the inbox's
      # own visibility gate. Pass ?include_hidden=true to list them anyway.
      def index
        scope = Current.workspace.tags
        scope = scope.visible unless ActiveModel::Type::Boolean.new.cast(params[:include_hidden])
        @pagy, tags = pagy(scope.by_name, limit: per_page)
        counts = EmailMessageTag.where(tag_id: tags.map(&:id)).group(:tag_id).count
        render_page(tags.map { |tag| TagSerializer.new(tag, email_count: counts[tag.id] || 0).as_json }, @pagy)
      end
    end
  end
end
