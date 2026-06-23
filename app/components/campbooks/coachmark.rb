# frozen_string_literal: true

module Campbooks
  # A one-time, non-modal coachmark: a caret popover that points at an on-page
  # target (found by CSS selector) with a soft pulsing highlight drawn over it,
  # to teach a feature in place. There is no dimming backdrop, so the target stays
  # fully interactive — tapping it both does its thing and dismisses the coachmark.
  #
  # Built on the tour foundation: the `coachmark` Stimulus controller waits for the
  # target (it may be lazy-loaded), positions + reveals the bubble, repositions on
  # scroll/resize, and on dismiss POSTs the tour key (User#dismiss_tour!) so it
  # greets the user only once. Presentational — copy is passed in; the host renders
  # it only when the tour isn't dismissed yet.
  class Coachmark < Campbooks::Base
    # @param tour_key [String] User#dismiss_tour! key (e.g. "home_rings")
    # @param anchor [String] CSS selector of the element to point at
    # @param title [String]
    # @param body [String]
    # @param cta [String] dismiss button label
    # @param placement [Symbol] :bottom (bubble below the anchor) or :top
    # @param union_children [Boolean] highlight the union of the anchor's children
    #   (for a wide container like the rings strip) instead of its own box
    def initialize(tour_key:, anchor:, title:, body:, cta:, placement: :bottom, union_children: false, **attrs)
      @tour_key = tour_key
      @anchor = anchor
      @title = title
      @body = body
      @cta = cta
      @placement = placement
      @union_children = union_children
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names("hidden", custom),
        data: {
          controller: "coachmark",
          coachmark_anchor_value: @anchor,
          coachmark_placement_value: @placement.to_s,
          coachmark_tour_key_value: @tour_key,
          coachmark_dismiss_url_value: "/tours/#{@tour_key}/dismiss",
          coachmark_union_children_value: @union_children
        },
        **@attrs
      ) do
        highlight
        bubble
      end
    end

    private

    # Pulsing ember outline the controller sizes/places over the target. Behind the
    # bubble, never intercepts clicks (the target underneath stays tappable).
    def highlight
      div(
        class: "pointer-events-none fixed left-0 top-0 z-[55] rounded-[1.25rem] opacity-0 transition-opacity duration-200 " \
               "ring-2 ring-offset-2 ring-offset-background animate-pulse",
        style: "--tw-ring-color: var(--ember-solid); box-shadow: var(--ember-glow);",
        data: { coachmark_target: "highlight" }
      )
    end

    def bubble
      div(
        class: "fixed left-0 top-0 z-[56] w-[min(20rem,calc(100vw-1.5rem))] rounded-2xl border border-border bg-card p-4 " \
               "text-left opacity-0 shadow-xl transition-opacity duration-200",
        role: "dialog", aria_label: @title,
        data: { coachmark_target: "bubble" }
      ) do
        caret
        div(class: "relative flex items-start gap-3") do
          span(class: "mt-0.5 flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-ember-gradient text-white shadow-ember") do
            tap_icon
          end
          div(class: "min-w-0 flex-1") do
            h3(class: "text-sm font-semibold text-foreground") { @title }
            p(class: "mt-1 text-sm leading-snug text-muted-foreground") { @body }
          end
        end
        div(class: "relative mt-3 flex justify-end") do
          button(
            type: "button",
            class: "rounded-lg bg-ember-gradient px-3 py-1.5 text-sm font-semibold text-white shadow-ember " \
                   "transition-transform duration-150 active:scale-[0.98] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400",
            data: { action: "click->coachmark#dismiss" }
          ) { @cta }
        end
      end
    end

    # Rotated square peeking out of the bubble edge toward the anchor; the controller
    # sets its offset along that edge. Only the two outward borders show so it reads
    # as a caret, not a diamond.
    def caret
      edge = @placement.to_sym == :top ? "border-b border-r -bottom-1.5" : "border-l border-t -top-1.5"
      div(
        class: class_names("absolute h-3 w-3 rotate-45 border-border bg-card", edge),
        data: { coachmark_target: "caret" }
      )
    end

    def tap_icon
      svg(class: "h-5 w-5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.8", aria_hidden: "true") do
        raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M15.042 21.672 13.684 16.6m0 0-2.51 2.225.569-9.47 5.227 7.917-3.286-.672ZM12 2.25V4.5m5.834.166-1.591 1.591M20.25 10.5H18M7.757 14.743l-1.59 1.59M6 10.5H3.75m4.007-4.243-1.59-1.59"/>'))
      end
    end
  end
end
