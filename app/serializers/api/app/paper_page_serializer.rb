# frozen_string_literal: true

module Api
  module App
    # Screen-shaped read model for the Paper surface. Bundles the active bucket's
    # paginated documents together with all bucket counts into one round-trip.
    class PaperPageSerializer
      def initialize(bucket:, bucket_counts:, documents:, pagy:)
        @bucket        = bucket
        @bucket_counts = bucket_counts
        @documents     = documents
        @pagy          = pagy
      end

      def as_json
        {
          bucket:        @bucket,
          bucket_counts: @bucket_counts,
          documents:     serialized_documents,
          meta:          meta
        }
      end

      private

      def serialized_documents
        @documents.map { |doc| Api::App::DocumentSerializer.new(doc).as_json }
      end

      def meta
        return nil unless @pagy

        {
          page:        @pagy.page,
          per_page:    @pagy.limit,
          total:       @pagy.count,
          total_pages: @pagy.pages
        }
      end
    end
  end
end
