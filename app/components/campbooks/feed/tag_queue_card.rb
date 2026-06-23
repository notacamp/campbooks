# frozen_string_literal: true

module Campbooks
  module Feed
    # A consecutive run of tag_suggestion items, stacked as one tight filing
    # queue (see Feed::Reader.group_timeline). The rows share one vertical rhythm
    # and carry NO hairline between them — the feed's divide-y still draws a single
    # divider *around* the whole block, separating the lightweight queue from the
    # heavier cards above and below. This is what makes a run read as "a quick
    # filing queue, not a wall of cards" (see Feed::TagSuggestionCard).
    class TagQueueCard < Campbooks::Base
      # @param items [Array<Hash>] ordered { item:, subject: } pairs, all tag_suggestion
      def initialize(items:)
        @items = items
      end

      def view_template
        # py-4: lighter breathing than a full card's py-9 — the queue is the
        # feed's lowest-weight unit, but still clears the divider above/below.
        # data-feed-focus-unit: the whole queue is one scroll-focus unit, so the
        # rows brighten/dim together rather than flickering one by one.
        div(class: "py-4", data: { feed_focus_unit: true }) do
          @items.each do |pair|
            # Keep each row's feed_item_<id> so a one-tap File / Not now still
            # turbo-removes just that row; data-feed-card keeps it a keyboard stop.
            div(
              id: "feed_item_#{pair[:item].id}",
              class: "animate-fade-in scroll-mt-24",
              data: { feed_card: true }
            ) do
              render Campbooks::Feed::TagSuggestionCard.new(item: pair[:item], subject: pair[:subject])
            end
          end
        end
      end
    end
  end
end
