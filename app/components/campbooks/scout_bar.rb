# frozen_string_literal: true

module Campbooks
  # The glass-docked Scout composer, pinned to the bottom of the content column
  # (DESIGN.md §5 "Scout bar"). Extracted from the home page so Home and the Now
  # page share one bar. For now it's a link to the full Scout surface; the inline
  # chat/command overlay is a follow-up PR.
  #
  #   render Campbooks::ScoutBar.new(placeholder: t(".ask_scout"), coach_anchor: true)   # Home (desktop only)
  #   render Campbooks::ScoutBar.new(placeholder: "…", mobile_placeholder: "…", mobile: true, keycap: true)  # Now
  #
  # @param href [String, nil] link target (defaults to scout_path)
  # @param placeholder [String] desktop input placeholder
  # @param mobile_placeholder [String, nil] mobile placeholder (defaults to placeholder)
  # @param mobile [Boolean] also render the mobile docked bar (above the bottom nav)
  # @param keycap [Boolean] show a ⌘K keycap before the send button (desktop)
  # @param coach_anchor [Boolean] mark the desktop bar with data-scout-coach-anchor
  # @param desktop_max_width [String] Tailwind max-width for the desktop bar column
  class ScoutBar < Campbooks::Base
    SPARK_SVG = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[22px] w-[22px]" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'
    SEND_SVG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" class="h-[15px] w-[15px]" aria-hidden="true"><path d="M22 2 11 13M22 2l-7 20-4-9-9-4z"/></svg>'

    def initialize(href: nil, placeholder:, mobile_placeholder: nil, mobile: false, keycap: false,
                   coach_anchor: false, desktop_max_width: "max-w-[600px]")
      @href = href
      @placeholder = placeholder
      @mobile_placeholder = mobile_placeholder || placeholder
      @mobile = mobile
      @keycap = keycap
      @coach_anchor = coach_anchor
      @desktop_max_width = desktop_max_width
    end

    def view_template
      desktop_bar
      mobile_bar if @mobile
    end

    private

    def href
      @href || helpers.scout_path
    end

    # Desktop: centered under the content column (lg:left-20 clears the nav rail),
    # pointer-events-none on the wrapper so the empty space stays click-through.
    def desktop_bar
      div(class: "pointer-events-none fixed inset-x-0 bottom-4 z-30 hidden px-4 lg:left-20 lg:flex lg:justify-center") do
        a(
          href: href,
          data: @coach_anchor ? { scout_coach_anchor: "" } : {},
          class: class_names(
            "pointer-events-auto flex w-full items-center gap-2.5 rounded-2xl border border-border bg-card/80 p-2 pl-3.5 shadow-lg backdrop-blur-xl transition hover:bg-card",
            @desktop_max_width
          )
        ) do
          spark
          span(class: "flex-1 truncate text-sm text-muted-foreground") { @placeholder }
          keycap_chip if @keycap
          send_button
        end
      end
    end

    # Mobile: docked above the bottom nav (h-16 + safe area), full width inset-x-4.
    def mobile_bar
      div(class: "pointer-events-none fixed inset-x-4 bottom-[calc(4rem+env(safe-area-inset-bottom)+0.75rem)] z-30 flex justify-center lg:hidden") do
        a(
          href: href,
          class: "pointer-events-auto flex h-[46px] w-full items-center gap-2.5 rounded-2xl border border-border bg-card/80 pl-3.5 pr-2 shadow-lg backdrop-blur-xl transition hover:bg-card"
        ) do
          spark
          span(class: "flex-1 truncate text-sm text-muted-foreground") { @mobile_placeholder }
          send_button(size: :sm)
        end
      end
    end

    def spark
      span(class: "flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK_SVG)) }
    end

    def keycap_chip
      span(class: "flex-shrink-0 rounded-md border border-border px-1.5 py-0.5 font-mono text-[11px] leading-none text-muted-foreground") { "⌘K" }
    end

    def send_button(size: :md)
      dims = size == :sm ? "h-[30px] w-[30px]" : "h-8 w-8"
      span(class: class_names("flex flex-shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground", dims)) do
        raw(safe(SEND_SVG))
      end
    end
  end
end
