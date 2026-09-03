# frozen_string_literal: true

module Campbooks
  module Now
    # The decision deck: a stack of the existing feed cards you clear one at a time
    # (the Rethink's "home is a stack you clear, not a feed you scroll"). Two peeking
    # back sheets, a "1 of N" counter, and the stack itself — the same
    # Campbooks::Feed::Card renders as the home feed, framed into a card by the
    # .now-deck-stack CSS (so an undo-reinjected card and load-more cards get the
    # frame too, without touching Feed::Card). Setup steps ride at the tail as cards.
    #
    # Only the top card shows; the now-deck Stimulus controller flies a card out on
    # action, rises the next, ticks the counter, lazily loads more, and reveals the
    # cleared block when the stack empties. A pre-connect user (:none) sees the
    # connect experience instead of a stack; an already-empty segment/inbox renders
    # the cleared/empty block outright.
    class Deck < Campbooks::Base
      # @param attention_pairs [Array<Hash>] presented { item:, subject: } — needs-you cards
      # @param timeline_pairs [Array<Hash>] presented ambient cards (page 1)
      # @param setup_items [Array<Hash>] localized SetupStatus incomplete items
      # @param inbox_state [Symbol] Home::InboxState — :none/:syncing/:disconnected/:caught_up
      # @param segment [Symbol] the active segment (all/priority/follow_ups/mail/time)
      # @param total [Integer] the segment's total card count (the counter's N)
      # @param next_page [Integer, nil] the deck's next timeline page (load-more)
      # @param has_more [Boolean] whether more timeline pages exist
      def initialize(attention_pairs:, timeline_pairs:, setup_items:, inbox_state:, segment:,
                     total:, next_page: nil, has_more: false)
        @attention_pairs = attention_pairs
        @timeline_pairs = timeline_pairs
        @setup_items = setup_items
        @inbox_state = inbox_state
        @segment = segment.to_sym
        @total = total.to_i
        @next_page = next_page
        @has_more = has_more
        @card_count = @attention_pairs.size + @timeline_pairs.size + @setup_items.size
      end

      def view_template
        if @inbox_state == :none
          div(class: "mt-6") { render Campbooks::InboxEmptyState.new(state: :none, wrapper_class: "mt-4") }
        elsif @card_count.positive?
          deck_stack
        else
          div(class: "mt-6") { cleared_block }
        end
      end

      private

      def deck_stack
        div(
          class: "now-deck relative mt-6 pt-3.5",
          data: {
            # feed-keyboard (→ primary, ← escape, letters) acts on the top card;
            # now-deck pins its data-focused and keeps it there. feed-focus is
            # deliberately NOT mounted — its scroll-dimming would fade the single
            # visible card, and now-deck owns the focus mark here instead.
            controller: "now-deck feed-keyboard",
            now_deck_segment_value: @segment,
            now_deck_total_value: @total,
            now_deck_url_value: helpers.now_path(segment: @segment),
            now_deck_counter_format_value: t(".counter")
          }
        ) do
          back_sheet(min: 2, klass: "left-3.5 right-3.5 top-0 opacity-55")
          back_sheet(min: 3, klass: "left-7 right-7 -top-3 opacity-30")
          counter
          stack
          pagination_state
          cleared_template
        end
      end

      def back_sheet(min:, klass:)
        div(
          class: class_names("pointer-events-none absolute h-10 rounded-[18px] border border-border bg-card", klass),
          data: { now_deck_target: "backSheet", now_deck_min: min },
          hidden: @card_count < min,
          aria_hidden: "true"
        )
      end

      def counter
        span(
          id: "now_deck_counter",
          class: "absolute right-6 top-8 z-20 text-xs tabular-nums text-muted-foreground sm:right-[26px]",
          data: { now_deck_target: "counter" }
        ) { t(".counter").sub("{n}", @total.to_s) }
      end

      # KEEP id="feed_timeline": Feed::ItemsController#undo prepends the restored
      # card here, so undo drops it back on top of the deck.
      def stack
        div(id: "feed_timeline", class: "now-deck-stack relative", data: { now_deck_target: "stack" }) do
          @attention_pairs.each { |pair| feed_card(pair) }
          @timeline_pairs.each { |pair| feed_card(pair) }
          @setup_items.each { |item| render Campbooks::Now::SetupCard.new(item: item) }
        end
      end

      def feed_card(pair)
        render Campbooks::Feed::Card.new(item: pair[:item], subject: pair[:subject])
      end

      # The now-deck controller reads this for lazy load-more and rewrites it after
      # each page. Hidden; carries the pagination cursor only.
      def pagination_state
        div(
          id: "now_deck_state",
          hidden: true,
          data: { now_deck_target: "state", next_page: @next_page, has_more: @has_more }
        )
      end

      # Rendered but hidden; the controller reveals it (and hides the stack) when the
      # last card leaves, so reaching the end is a moment without a round-trip.
      def cleared_template
        div(id: "now_deck_cleared", hidden: true, data: { now_deck_target: "cleared" }) { cleared_block }
      end

      def cleared_block
        if @segment != :all
          segment_empty_block
        elsif %i[syncing disconnected].include?(@inbox_state)
          render Campbooks::InboxEmptyState.new(state: @inbox_state, wrapper_class: "mt-2")
        else
          stack_cleared_block
        end
      end

      def stack_cleared_block
        div(class: "mt-2 flex flex-col items-center text-center") do
          render Campbooks::ScoutAvatar.new(size: :xl)
          h2(class: "mt-4 text-lg font-semibold text-foreground") { t(".cleared_title") }
          p(class: "mt-1 max-w-xs text-sm leading-relaxed text-muted-foreground") { t(".cleared_body") }
        end
      end

      def segment_empty_block
        div(class: "mt-2 flex flex-col items-center text-center") do
          render Campbooks::ScoutAvatar.new(size: :xl)
          h2(class: "mt-4 text-lg font-semibold text-foreground") { t(".segment_empty_title") }
          a(
            href: helpers.now_path,
            data: { turbo_frame: "now_deck", turbo_action: "advance" },
            class: "mt-3 text-sm font-medium text-foreground underline decoration-border underline-offset-4 hover:decoration-foreground"
          ) { t(".show_everything") }
        end
      end
    end
  end
end
