# frozen_string_literal: true

module Campbooks
  # The compact Scout launcher: a floating Ember-spark button that opens the
  # global Scout overlay. Rendered by the email layout (inbox / compose desk),
  # where the panes are full-height with their own bottom composers and there's
  # no room for the full docked ScoutBar. Bold layout only; never on /scout.
  #
  # Sits bottom-right on desktop; on mobile it floats just above the bottom nav
  # (clear of the safe-area inset). Clicking it fires `scout-overlay#open`.
  #
  #   render Campbooks::ScoutLauncher.new
  class ScoutLauncher < Campbooks::Base
    SPARK_SVG = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-6 w-6" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'

    def view_template
      div(class: "pointer-events-none fixed right-4 bottom-[calc(4rem+env(safe-area-inset-bottom)+0.75rem)] z-30 lg:bottom-4") do
        button(
          type: "button",
          data: { action: "scout-overlay#open" },
          aria: { label: t(".aria_label") },
          class: "pointer-events-auto flex h-11 w-11 items-center justify-center rounded-full border border-border bg-card/80 text-[color:var(--ember-solid)] shadow-lg backdrop-blur-xl transition hover:bg-card hover:shadow-xl active:scale-95"
        ) { raw(safe(SPARK_SVG)) }
      end
    end
  end
end
