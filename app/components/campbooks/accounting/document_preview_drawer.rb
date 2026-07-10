# frozen_string_literal: true

module Campbooks
  module Accounting
    # In-place document preview for the reconciliation workbench: a right-hand
    # slide-over (bottom sheet on mobile) with the document file in an iframe,
    # so users can inspect an invoice without leaving the page. Opened by the
    # `document-preview` Stimulus controller, which intercepts plain clicks on
    # /documents/:id links inside the workbench (modified clicks and the
    # drawer's own "open full page" link still navigate normally).
    #
    # Rendered ONCE per page, outside #reconciliation_content so Turbo Stream
    # broadcasts never replace it while open.
    class DocumentPreviewDrawer < Campbooks::Base
      def view_template
        div(data: { controller: "document-preview" }) do
          div(
            class: "fixed inset-0 z-30 bg-black/40",
            data: { document_preview_target: "backdrop", action: "click->document-preview#close" },
            style: "display: none"
          )

          div(
            class: class_names(
              "fixed z-40 bg-card shadow-2xl border border-border flex flex-col",
              "transition-transform duration-300 ease-out",
              # Mobile: bottom sheet. Desktop (sm+): full-height right panel.
              "inset-x-0 bottom-0 h-[85vh] rounded-t-2xl translate-y-full",
              "sm:inset-x-auto sm:inset-y-0 sm:right-0 sm:h-full sm:w-full sm:max-w-2xl",
              "sm:rounded-none sm:translate-y-0 sm:translate-x-full",
              "invisible"
            ),
            data: { document_preview_target: "panel" },
            role: "dialog",
            aria_modal: "true",
            aria_label: t(".aria_label")
          ) do
            header_bar
            iframe(
              src: "about:blank",
              class: "flex-1 w-full bg-muted/30",
              title: t(".iframe_title"),
              data: { document_preview_target: "iframe" }
            )
          end
        end
      end

      private

      def header_bar
        div(class: "flex items-center justify-between gap-3 px-4 py-3 border-b border-border shrink-0") do
          p(class: "text-sm font-semibold truncate min-w-0", data: { document_preview_target: "title" }) { "" }
          div(class: "flex items-center gap-1 shrink-0") do
            a(
              href: "#",
              target: "_blank",
              rel: "noopener",
              class: "text-xs font-medium text-accent-600 hover:text-accent-700 px-2 py-1",
              data: { document_preview_target: "fullPageLink", document_preview_skip: "true" }
            ) { t(".open_full_page") }
            button(
              type: "button",
              class: "p-1.5 rounded-md text-muted-foreground hover:bg-muted hover:text-foreground",
              data: { action: "click->document-preview#close" },
              aria_label: t(".close")
            ) do
              svg(viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2",
                  class: "w-4 h-4") do |s|
                s.path(stroke_linecap: "round", stroke_linejoin: "round", d: "M6 18L18 6M6 6l12 12")
              end
            end
          end
        end
      end
    end
  end
end
