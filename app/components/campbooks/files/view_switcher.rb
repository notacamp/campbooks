# frozen_string_literal: true

module Campbooks
  module Files
    # Segmented control that switches the Files list between LAYOUT modes — List
    # (the current table / mobile cards) and Grid (thumbnail tiles). Driven by
    # the `files-layout` Stimulus controller, which persists the choice in
    # localStorage and syncs the active segment client-side, so there is no
    # server-side `active` argument (mirrors Campbooks::InboxViewSwitcher).
    class ViewSwitcher < Campbooks::Base
      LAYOUTS = %i[list grid].freeze

      ICONS = {
        # bulleted rows
        list: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8.25 6.75h12M8.25 12h12M8.25 17.25h12M3.75 6.75h.008v.008H3.75V6.75zM3.75 12h.008v.008H3.75V12zM3.75 17.25h.008v.008H3.75v-.008z"/>',
        # four tiles
        grid: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM13.5 13.5h6v6h-6z"/>'
      }.freeze

      def initialize(**attrs)
        @attrs = attrs
      end

      def view_template
        custom_class = @attrs.delete(:class)
        div(
          class: class_names("inline-flex items-center gap-0.5 rounded-lg bg-muted p-0.5", custom_class),
          role: "group",
          aria: { label: t(".aria_label") },
          **@attrs
        ) do
          LAYOUTS.each { |layout| segment(layout) }
        end
      end

      private

      def segment(layout)
        button(
          type: "button",
          class: "inline-flex items-center justify-center rounded-md p-1.5 transition-colors cursor-pointer text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-500",
          title: t(".#{layout}"),
          aria: { label: t(".#{layout}"), pressed: "false" },
          data: {
            files_layout_target: "button",
            layout: layout,
            action: "click->files-layout#select"
          }
        ) do
          svg(class: "w-4 h-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
            raw(safe(ICONS[layout]))
          end
        end
      end
    end
  end
end
