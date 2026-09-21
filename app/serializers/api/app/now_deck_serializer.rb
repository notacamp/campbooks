# frozen_string_literal: true

module Api
  module App
    # Serializes the full Now deck screen payload. The SPA receives everything
    # it needs for the initial render in one round trip.
    class NowDeckSerializer
      def initialize(segment:, segment_counts:, attention_pairs:, timeline_pairs:, timeline_pagy:,
                     ledger:, log:, inbox_state:)
        @segment        = segment
        @segment_counts = segment_counts
        @attention_pairs = attention_pairs
        @timeline_pairs  = timeline_pairs
        @timeline_pagy   = timeline_pagy
        @ledger          = ledger
        @log             = log
        @inbox_state     = inbox_state
      end

      def as_json
        {
          segment:        @segment,
          segment_counts: @segment_counts,
          attention:      serialize_pairs(@attention_pairs),
          timeline:       serialize_pairs(@timeline_pairs),
          timeline_meta:  timeline_meta,
          ledger:         ledger_data,
          log:            log_data,
          inbox_state:    @inbox_state
        }
      end

      private

      def serialize_pairs(pairs)
        Array(pairs).map do |pair|
          Api::App::FeedItemSerializer.new(pair[:item], subject: pair[:subject]).as_json
        end
      end

      def timeline_meta
        return {} unless @timeline_pagy

        {
          page:       @timeline_pagy.page,
          per_page:   @timeline_pagy.limit,
          next_page:  @timeline_pagy.next
        }
      rescue StandardError
        {}
      end

      def ledger_data
        return {} unless @ledger

        {
          buckets:          @ledger.buckets,
          total:            @ledger.total,
          need_you:         @ledger.need_you,
          archived_by_rules: @ledger.archived_by_rules?
        }
      rescue StandardError
        {}
      end

      def log_data
        return {} unless @log

        {
          count:  @log.count,
          events: @log.events.map { |e| event_row(e) }
        }
      rescue StandardError
        {}
      end

      def event_row(event)
        {
          id:          event.id,
          name:        event.name,
          occurred_at: event.occurred_at&.iso8601,
          payload:     event.payload
        }
      rescue StandardError
        { id: event.id }
      end
    end
  end
end
