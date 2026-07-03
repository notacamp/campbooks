# frozen_string_literal: true

module Campbooks
  module Compose
    # The parked-draft pill: a small bottom-right capsule that survives
    # navigation (rendered by the layout whenever an unsent draft exists) and
    # re-opens the draft in the Dock on click. Sits above the mobile bottom nav.
    class Pill < Campbooks::Base
      def initialize(draft:)
        @draft = draft
      end

      def view_template
        a(href: helpers.draft_email_path(@draft),
          id: "compose_draft_pill",
          class: "fixed bottom-20 lg:bottom-5 right-4 z-[60] flex items-center gap-2 max-w-[240px] " \
                 "bg-card border border-gray-200 rounded-full pl-2.5 pr-3.5 py-2 shadow-lg " \
                 "hover:shadow-xl hover:-translate-y-0.5 transition-all motion-reduce:transition-none",
          data: { turbo_stream: "true", compose_dock_target: "pill" },
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

      private

      def title_text
        @draft.display_title.presence || t(".untitled")
      end
    end
  end
end
