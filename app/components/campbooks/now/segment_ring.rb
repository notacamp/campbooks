# frozen_string_literal: true

module Campbooks
  module Now
    # One segment ring on the Now page: the same Instagram-story ring the inbox
    # Skim tray uses (Campbooks::SkimRing's drawing — 2.5px Ember ring + glow, a
    # 56px disc, a count badge, an 11px label), reframed as a filter over the one
    # decision deck. Tapping a ring re-renders the deck for that segment in place
    # (it targets the now_deck turbo-frame). The Docs ring is the exception: it's a
    # button that opens the document review overlay, so it takes no href.
    #
    # States: default (Ember ring), active (the current segment — inner disc filled
    # bg-secondary, label emphasised), done (count 0 — gray ring, no glow, a check
    # in place of the count, muted label).
    class SegmentRing < Campbooks::Base
      # Ring glyphs from the approved mock (icons.svg.txt), stroked to match.
      ICONS = {
        all:        '<path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/>',
        priority:   '<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>',
        follow_ups: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
        mail:       '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
        time:       '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>',
        docs:       '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>'
      }.freeze

      CHECK_SVG = '<svg class="h-[11px] w-[11px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>'

      # @param segment [Symbol] one of ICONS' keys — picks the glyph
      # @param label [String] text under the ring
      # @param count [Integer] badge count (0 → the "done" check)
      # @param active [Boolean] the currently-selected segment
      # @param href [String, nil] link target; nil renders a <button> (Docs overlay)
      def initialize(segment:, label:, count: 0, active: false, href: nil, **attrs)
        @segment = segment.to_sym
        @label = label
        @count = count.to_i
        @active = active
        @href = href
        @attrs = attrs
      end

      def view_template
        custom = @attrs.delete(:class)
        tag = @href ? :a : :button
        base = {
          class: class_names("group flex w-[4.25rem] flex-shrink-0 snap-start flex-col items-center gap-1.5 outline-none", custom),
          **@attrs
        }
        base[:href] = @href if @href
        base[:type] = "button" unless @href

        public_send(tag, **base) do
          div(class: "relative") do
            div(class: class_names(
              "rounded-full p-[2.5px] transition-transform duration-150 group-hover:scale-105 group-active:scale-95 group-focus-visible:ring-2 group-focus-visible:ring-offset-2 group-focus-visible:ring-ring",
              done? ? "bg-border" : "bg-ember-gradient shadow-ember"
            )) do
              div(class: class_names(
                "flex h-14 w-14 items-center justify-center rounded-full",
                @active ? "bg-secondary" : "bg-card"
              )) do
                svg(class: class_names("h-6 w-6", done? ? "text-muted-foreground" : "text-foreground"),
                    fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.9",
                    stroke_linecap: "round", stroke_linejoin: "round", aria_hidden: "true") do
                  raw(safe(ICONS.fetch(@segment, ICONS[:all])))
                end
              end
            end
            badge
          end
          span(class: class_names(
            "max-w-full truncate text-[11px]",
            if @active then "font-semibold text-foreground"
            elsif done? then "font-medium text-muted-foreground"
            else "font-medium text-gray-600 dark:text-gray-300"
            end
          )) { @label }
        end
      end

      private

      def done?
        @count.zero?
      end

      def badge
        span(class: class_names(
          "absolute -bottom-0.5 -right-0.5 inline-flex min-w-[18px] items-center justify-center rounded-full border-2 border-card px-1 text-[10px] font-semibold leading-none tabular-nums",
          done? ? "bg-muted text-muted-foreground" : "bg-primary text-primary-foreground"
        )) do
          done? ? raw(safe(CHECK_SVG)) : plain(abbreviated_count)
        end
      end

      def abbreviated_count
        @count < 1000 ? @count.to_s : "#{(@count / 1000.0).round(1)}k"
      end
    end
  end
end
