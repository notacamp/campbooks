# frozen_string_literal: true

module Api
  module V1
    # Lists the workspace's document types (used to classify documents). Small
    # reference set — returned unpaginated.
    class DocumentTypesController < BaseController
      before_action -> { doorkeeper_authorize! :"document_types:read" }, only: [ :index ]

      def index
        types = Current.workspace.document_types.order(:category, :name)
        render_data(types.map { |type| DocumentTypeSerializer.new(type).as_json })
      end
    end
  end
end
