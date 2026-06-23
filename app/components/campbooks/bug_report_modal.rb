# frozen_string_literal: true

module Campbooks
  # The "Report a bug" drawer. Rendered ONCE per layout (near the other shell
  # singletons), it owns the whole flow via the `bug-report` Stimulus
  # controller: the controller catches clicks on any `[data-bug-report-open]`
  # trigger (see Campbooks::BugReportButton in the nav), captures page context +
  # an optional screenshot, and POSTs to BugReportsController.
  #
  # Visually it is a black drawer that slides in from the right edge, anchored
  # at the same spot as the floating Campbooks::BugReportTab (`bottom-24` /
  # `lg:bottom-12`) so it appears to slide out of that folder tab — Hotjar-style.
  # The panel is opaque and flush to the right edge, so it covers the tab while
  # open.
  #
  # It is rendered at the layout root on purpose — the mobile topbar uses
  # `backdrop-blur`, which would create a containing block and break a
  # `position: fixed` overlay nested inside it.
  #
  #   render(Campbooks::BugReportModal.new)
  class BugReportModal < Campbooks::Base
    BUG_SVG = '<svg class="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m8 2 1.88 1.88"/><path d="M14.12 3.88 16 2"/><path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1"/><path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6"/><path d="M12 20v-9"/><path d="M6.53 9C4.6 8.8 3 7.1 3 5"/><path d="M6 13H2"/><path d="M3 21c0-2.1 1.7-3.9 3.8-4"/><path d="M20.97 5c0 2.1-1.6 3.8-3.5 4"/><path d="M22 13h-4"/><path d="M17.2 17c2.1.1 3.8 1.9 3.8 4"/></svg>'
    CLOSE_SVG = '<svg class="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>'
    CHECK_SVG = '<svg class="size-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21.801 10A10 10 0 1 1 17 3.335"/><path d="m9 11 3 3L22 4"/></svg>'

    # @param open [Boolean] render visible immediately (used by the Lookbook
    #   preview). Production renders hidden; the `bug-report` controller toggles
    #   the visibility + slide/fade classes.
    def initialize(open: false)
      @open = open
    end

    def view_template
      div(
        data: {
          controller: "bug-report",
          bug_report_submitting_text_value: t(".submitting"),
          bug_report_error_generic_value: t(".error_generic"),
          bug_report_error_empty_value: t(".error_empty")
        }
      ) do
        overlay
      end
    end

    private

    def overlay
      div(
        class: class_names("fixed inset-0 z-[60]", ("hidden" unless @open)),
        data: {
          bug_report_target: "overlay",
          action: "keydown.esc@window->bug-report#close"
        }
      ) do
        # Dimming scrim — clicking it closes the drawer.
        div(
          class: class_names(
            "absolute inset-0 bg-black/50 backdrop-blur-sm transition-opacity duration-300 ease-out",
            @open ? "opacity-100" : "opacity-0"
          ),
          data: { bug_report_target: "backdrop", action: "click->bug-report#close" }
        )

        # The black drawer. Anchored to the right edge at the tab's height and
        # rounded only on the left, so it reads as the tab sliding open.
        div(
          class: class_names(
            "absolute bottom-24 right-0 flex max-h-[80vh] w-[calc(100%-1rem)] max-w-md flex-col " \
            "overflow-hidden rounded-l-2xl border-y border-l border-white/10 bg-neutral-950 " \
            "text-neutral-100 shadow-2xl transition-transform duration-300 ease-out lg:bottom-12",
            @open ? "translate-x-0" : "translate-x-full"
          ),
          data: { bug_report_target: "panel" },
          role: "dialog",
          aria_modal: "true",
          aria_label: t(".title")
        ) do
          form_view
          success_view
        end
      end
    end

    def form_view
      div(data: { bug_report_target: "formView" }) do
        # Header
        div(class: "flex items-center justify-between gap-3 border-b border-white/10 px-5 py-4") do
          div(class: "flex items-center gap-2.5") do
            span(class: "flex size-9 items-center justify-center rounded-lg bg-white/10 text-white") { raw(safe(BUG_SVG)) }
            div do
              h2(class: "text-base font-semibold leading-tight text-white") { t(".title") }
              p(class: "text-xs text-neutral-400") { t(".subtitle") }
            end
          end
          button(
            type: "button",
            aria_label: t(".close"),
            class: "flex size-8 shrink-0 items-center justify-center rounded-lg text-neutral-400 transition-colors hover:bg-white/10 hover:text-white cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/30",
            data: { action: "click->bug-report#close" }
          ) { raw(safe(CLOSE_SVG)) }
        end

        # Body — a real form so the no-JS path still submits.
        form(
          action: helpers.bug_reports_path,
          method: "post",
          accept_charset: "UTF-8",
          data: { bug_report_target: "form", action: "submit->bug-report#submit" }
        ) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "page_url", data: { bug_report_target: "pageUrl" })
          input(type: "hidden", name: "metadata", data: { bug_report_target: "metadata" })

          div(class: "space-y-4 px-5 py-5") do
            div do
              label(for: "bug-report-description", class: "mb-1.5 block text-sm font-medium text-neutral-200") { t(".description_label") }
              textarea(
                id: "bug-report-description",
                name: "description",
                rows: "4",
                required: true,
                maxlength: "5000",
                placeholder: t(".description_placeholder"),
                class: "block w-full resize-y rounded-lg border border-white/15 bg-white/5 px-3 py-2 text-sm text-white shadow-sm placeholder:text-neutral-500 focus:border-white/30 focus:outline-none focus:ring-2 focus:ring-white/20",
                data: { bug_report_target: "description", action: "input->bug-report#clearError" }
              ) { "" }
            end

            # Screenshot opt-in
            label(class: "flex cursor-pointer items-start gap-2.5") do
              input(
                type: "checkbox",
                checked: true,
                class: "mt-0.5 size-4 rounded border-white/20 accent-white focus:ring-2 focus:ring-white/30",
                data: { bug_report_target: "screenshotToggle" }
              )
              span(class: "text-sm text-neutral-200") { t(".screenshot_label") }
            end

            # What we attach — keep the user informed (privacy).
            p(class: "text-xs leading-relaxed text-neutral-400") { t(".context_note") }

            # Error region
            p(
              class: "hidden text-sm font-medium text-red-400",
              role: "alert",
              data: { bug_report_target: "error" }
            )
          end

          # Footer
          div(class: "flex items-center justify-end gap-2 border-t border-white/10 px-5 py-3") do
            button(
              type: "button",
              class: "cursor-pointer rounded-md px-3 py-1.5 text-sm font-medium text-neutral-300 transition-colors hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/30",
              data: { action: "click->bug-report#close" }
            ) { t(".cancel") }
            button(
              type: "submit",
              class: "cursor-pointer rounded-md bg-white px-3 py-1.5 text-sm font-medium text-neutral-950 shadow-sm transition-colors hover:bg-neutral-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 disabled:opacity-50",
              data: { bug_report_target: "submit" }
            ) { t(".submit") }
          end
        end
      end
    end

    def success_view
      div(class: "hidden px-5 py-8 text-center", data: { bug_report_target: "successView" }) do
        div(class: "mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-white/10 text-white") { raw(safe(CHECK_SVG)) }
        h2(class: "text-base font-semibold text-white") { t(".success_title") }
        p(class: "mx-auto mt-1.5 max-w-xs text-sm text-neutral-400") { t(".success_body") }
        div(class: "mt-5") do
          button(
            type: "button",
            class: "cursor-pointer rounded-md bg-white px-3 py-1.5 text-sm font-medium text-neutral-950 shadow-sm transition-colors hover:bg-neutral-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40",
            data: { action: "click->bug-report#close" }
          ) { t(".done") }
        end
      end
    end
  end
end
