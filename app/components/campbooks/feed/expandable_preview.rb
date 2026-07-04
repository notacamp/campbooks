# frozen_string_literal: true

module Campbooks
  module Feed
    # The "peek inside" disclosure on email-backed feed cards: a collapsed
    # <details> whose body is a lazy turbo-frame into Feed::ItemsController#preview,
    # so the full email renders in place only when opened — making the call
    # (archive / file / reply / confirm) never requires leaving the feed. Native
    # disclosure semantics give keyboard + screen-reader support with no JS; the
    # frame's loading=lazy defers the fetch until the details actually opens
    # (hidden frames don't intersect the viewport).
    class ExpandablePreview < Campbooks::Base
      register_element :turbo_frame

      # `label:` overrides the collapsed text — cards whose subject is not the
      # email itself (reminder, task) say "Show source email" instead.
      def initialize(item:, label: nil, **attrs)
        @item = item
        @label = label
        @attrs = attrs
      end

      def view_template
        details(class: class_names("group/preview", @attrs.delete(:class)), **@attrs) do
          summary(
            class: "inline-flex w-fit cursor-pointer select-none list-none items-center gap-1 rounded-md " \
                   "text-[12px] font-medium text-muted-foreground outline-none transition-colors " \
                   "hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring " \
                   "[&::-webkit-details-marker]:hidden"
          ) do
            chevron
            span(class: "group-open/preview:hidden") { @label || t(".show") }
            span(class: "hidden group-open/preview:inline") { t(".hide") }
          end
          turbo_frame(
            id: "feed_item_#{@item.id}_preview",
            src: helpers.preview_feed_item_path(@item),
            loading: "lazy",
            class: "block"
          ) { skeleton }
        end
      end

      private

      def chevron
        svg(
          class: "h-3.5 w-3.5 transition-transform duration-150 group-open/preview:rotate-90",
          viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2",
          stroke_linecap: "round", stroke_linejoin: "round", aria_hidden: "true"
        ) { raw safe(%(<path d="m9 18 6-6-6-6"/>)) }
      end

      # Placeholder shown between opening the disclosure and the frame arriving.
      def skeleton
        div(class: "mt-2 space-y-2", aria_hidden: "true") do
          div(class: "h-3 w-3/4 animate-pulse rounded bg-muted")
          div(class: "h-3 w-1/2 animate-pulse rounded bg-muted")
        end
      end
    end
  end
end
