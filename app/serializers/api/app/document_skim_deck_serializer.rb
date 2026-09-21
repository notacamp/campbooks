# frozen_string_literal: true

module Api
  module App
    # Serializes the Document Skim deck: an array of category rings each carrying
    # per-document cards (built by Documents::SkimBuilder).
    class DocumentSkimDeckSerializer
      def initialize(rings)
        @rings = rings
      end

      def as_json
        {
          rings: @rings.map { |ring| serialize_ring(ring) },
          total: @rings.sum { |r| r[:count].to_i }
        }
      end

      private

      def serialize_ring(ring)
        {
          category: ring[:category],
          label:    ring[:label],
          count:    ring[:count],
          cards:    ring[:clusters].map { |card| serialize_card(card) }
        }
      end

      def serialize_card(card)
        card.slice(
          :document_id, :category, :display_title, :entity_display_name,
          :reference_display, :document_date, :amount_display, :ai_confidence_score,
          :type_label, :type_color, :type_id, :is_image, :is_pdf, :filename,
          :extracted_fields, :title_value, :position, :total
        )
      end
    end
  end
end
