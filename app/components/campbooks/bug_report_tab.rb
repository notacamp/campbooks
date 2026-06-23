# frozen_string_literal: true

module Campbooks
  # The floating "Report a bug" tab — a folder-divider-style tab fixed to the
  # right edge of the viewport, low on the page (bottom-right). Like
  # Campbooks::BugReportButton it is just a `data-bug-report-open` trigger, so
  # the one Campbooks::BugReportModal catches the click and opens in place — the
  # reporter screenshots the current screen, so users report a bug about what
  # they're looking at without navigating away.
  #
  # `z-50` keeps it above page chrome (e.g. the documents skim swipe zones) while
  # staying below the command palette / modal. Sits above the mobile bottom nav.
  # Rendered once per authenticated layout.
  #
  #   render(Campbooks::BugReportTab.new)
  class BugReportTab < Campbooks::Base
    def view_template
      button(
        type: "button",
        data: { bug_report_open: "" },
        aria_label: label,
        class: "group fixed bottom-24 right-0 z-50 flex flex-col items-center gap-2 rounded-l-xl bg-foreground py-3.5 pl-2.5 pr-2 text-background shadow-lg ring-1 ring-border/20 transition-[padding] duration-150 hover:pr-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring lg:bottom-12 print:hidden"
      ) do
        raw(safe(Campbooks::BugReportButton.bug_svg("size-5")))
        span(class: "text-[11px] font-semibold uppercase tracking-wide [writing-mode:vertical-rl] rotate-180") { label }
      end
    end

    private

    # Reuse the trigger's translated label rather than minting a second key.
    def label
      t("components.bug_report_button.label")
    end
  end
end
