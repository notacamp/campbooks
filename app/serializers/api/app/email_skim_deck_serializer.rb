# frozen_string_literal: true

module Api
  module App
    # Serializes the Email Skim deck: an array of theme rings with cluster cards,
    # as built by Emails::SkimDeck.for → Emails::SkimBuilder.
    class EmailSkimDeckSerializer
      def initialize(rings)
        @rings = rings
      end

      def as_json
        {
          rings: @rings.map { |ring| serialize_ring(ring) },
          total: @rings.sum { |r| (r[:clusters]&.size || 0) }
        }
      end

      private

      def serialize_ring(ring)
        {
          theme:    ring[:theme],
          label:    ring[:label],
          summary:  ring[:summary],
          count:    ring[:clusters]&.size || 0,
          clusters: (ring[:clusters] || []).map { |cluster| serialize_cluster(cluster) }
        }
      end

      def serialize_cluster(cluster)
        # SkimBuilder clusters are plain hashes; pass through the SPA-usable keys.
        {
          id:           cluster[:id],
          theme:        cluster[:theme],
          subject:      cluster[:subject],
          from_name:    cluster[:from_name],
          from_address: cluster[:from_address],
          sender_kind:  cluster[:sender_kind],
          snippet:      cluster[:snippet],
          received_at:  cluster[:received_at]&.iso8601,
          email_ids:    cluster[:email_ids],
          count:        cluster[:count],
          is_follow_up: cluster[:is_follow_up],
          follow_up_reason: cluster[:follow_up_reason],
          suggested_action: cluster[:suggested_action]
        }
      end
    end
  end
end
