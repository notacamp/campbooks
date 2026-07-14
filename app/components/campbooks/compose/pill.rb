# frozen_string_literal: true

module Campbooks
  module Compose
    # The parked-draft pill: a small bottom-right capsule that survives
    # navigation (rendered by the layout whenever an unsent draft exists) and
    # re-opens the draft in the Dock on click. Sits above the mobile bottom nav.
    # The × dismisses the pill without deleting the draft (server-side
    # dismissed_at + Undo toast); editing the draft again revives it.
    class Pill < Campbooks::Base
      def initialize(draft:)
        @draft = draft
      end

      def view_template
        div(id: "compose_draft_pill",
            class: "fixed bottom-20 lg:bottom-5 right-4 z-[60] flex items-center max-w-[250px] " \
                   "bg-card border border-gray-200 rounded-full pl-2.5 pr-1.5 py-1.5 shadow-lg " \
                   "hover:shadow-xl hover:-translate-y-0.5 transition-all motion-reduce:transition-none",
            data: { compose_dock_target: "pill" }) do
          resume_link
          dismiss_button
        end
      end

      private

      def resume_link
        a(href: helpers.draft_email_path(@draft),
          class: "flex items-center gap-2 min-w-0 py-0.5",
          data: { turbo_stream: "true" },
          aria_label: t(".resume_aria")) do
          span(class: "w-5 h-5 rounded-full bg-accent-600 text-white flex items-center justify-center flex-shrink-0") do
            svg(class: "w-2.5 h-2.5", fill: "none", stroke: "currentColor", stroke_width: "2.2",
                stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
              raw(safe('<path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"/>'))
            end
          end
          span(class: "min-w-0") do
            span(class: "block text-xs font-medium text-gray-800 truncate") { title_text }
            span(class: "block text-[10px] text-gray-400") { t(".draft_saved") }
          end
        end
      end

      # Mirrors ActionToast#undo_button: a tiny inline form so the × can POST
      # (with CSRF) as a Turbo Stream from any page the pill renders on.
      def dismiss_button
        form(action: helpers.dismiss_draft_email_path(@draft), method: :post, class: "contents") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: "w-6 h-6 ml-1 rounded-full flex items-center justify-center flex-shrink-0 " \
                        "text-gray-400 hover:text-gray-600 hover:bg-gray-100 transition-colors cursor-pointer",
                 title: t(".dismiss_title"), aria_label: t(".dismiss_title")) do
            svg(class: "w-3 h-3", fill: "none", stroke: "currentColor", stroke_width: "2.2",
                stroke_linecap: "round", viewBox: "0 0 24 24") do
              raw(safe('<path d="M18 6 6 18"/><path d="m6 6 12 12"/>'))
            end
          end
        end
      end

      def title_text
        @draft.display_title.presence || t(".untitled")
      end
    end
  end
end
