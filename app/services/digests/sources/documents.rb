# frozen_string_literal: true

module Digests
  module Sources
    # Gathers Documents received in the lookback period. Scoped to the workspace
    # via FolderAccessible (Document.accessible_to). When document_types is
    # configured, filters by joining the classification (DocumentType) table and
    # matching on lowercase name.
    class Documents < Base
      def self.direction = :lookback

      def items(period)
        scope = workspace.documents
                         .accessible_to(user)
                         .where(created_at: period.begin..period.end)

        type_names = Array(source_config["document_types"]).map(&:to_s).reject(&:blank?)
        if type_names.any?
          scope = scope.joins(:classification)
                       .where("LOWER(document_types.name) IN (?)", type_names.map(&:downcase))
        end

        scope.order(created_at: :desc).limit(MAX_ITEMS).map do |doc|
          Digests::Item.new(
            source_type: "document",
            source_id:   doc.id,
            title:       doc.display_title.to_s.presence || doc.original_filename.to_s,
            subtitle:    document_subtitle(doc),
            summary:     truncate(doc.description.to_s),
            timestamp:   doc.created_at&.iso8601
          )
        end
      end

      private

      def document_subtitle(doc)
        parts = []
        parts << doc.classification&.name if doc.classification.present?
        if doc.amount_cents.present? && doc.amount_cents != 0
          parts << doc.amount&.format
        end
        parts.join(" · ")
      end
    end
  end
end
