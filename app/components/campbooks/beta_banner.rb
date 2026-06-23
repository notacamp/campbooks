# frozen_string_literal: true

module Campbooks
  # Global "early beta / unstable" stripe pinned to the top of the app shell.
  #
  # Sets honest expectations on every page: Campbooks is in active beta, so
  # things can still break or change. Cloud-only (rendered behind
  # ApplicationController#show_beta_banner?, gated on `!self_hosted?`) and
  # dismissible — the `beta-banner` controller drops a cookie that the same
  # helper reads, so once closed it never renders again (no flash on later
  # visits) and the controller also removes it in-place for the current page.
  #
  #   <%= render(Campbooks::BetaBanner.new) if show_beta_banner? %>
  class BetaBanner < Campbooks::Base
    WARNING_SVG = '<svg class="size-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><path d="M12 9v4"/><path d="M12 17h.01"/></svg>'

    CLOSE_SVG = '<svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>'

    def view_template
      div(
        class: class_names(
          # px-10 keeps the centred message clear of the absolutely-placed
          # dismiss button on both sides; the stripe stays full-width and wraps
          # gracefully down to 375px.
          "relative w-full border-b px-10 py-2 text-center",
          "bg-amber-50 text-amber-900 border-amber-200",
          "dark:bg-amber-500/10 dark:text-amber-200 dark:border-amber-500/30"
        ),
        role: "status",
        data: { controller: "beta-banner" }
      ) do
        span(class: "inline-flex items-center justify-center gap-2 text-xs font-medium sm:text-sm") do
          raw(safe(WARNING_SVG))
          span { t(".message") }
        end

        button(
          type: "button",
          aria_label: t(".dismiss"),
          data: { action: "beta-banner#dismiss" },
          class: class_names(
            "absolute right-1.5 top-1/2 inline-flex size-7 -translate-y-1/2 items-center justify-center rounded-md",
            "cursor-pointer text-amber-700/80 transition-colors hover:bg-amber-100 hover:text-amber-900",
            "dark:text-amber-200/80 dark:hover:bg-amber-500/15 dark:hover:text-amber-100",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500/40"
          )
        ) { raw(safe(CLOSE_SVG)) }
      end
    end
  end
end
