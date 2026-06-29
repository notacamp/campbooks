# frozen_string_literal: true

module Campbooks
  # Segmented control that switches the inbox between LAYOUT modes — Default
  # (multi-pane), List (full-width + bottom-right drawer) and Board (status
  # kanban). Driven by the `inbox-layout` Stimulus controller: the active
  # segment is synced client-side from localStorage, so there is no server-side
  # `active` argument. This is a separate axis from the density picker
  # (compact/default/breathable) in the inbox settings modal.
  class InboxViewSwitcher < Campbooks::Base
    LAYOUTS = %i[default list board].freeze

    # Heroicons-style 24px line icons, rendered inside a shared <svg> wrapper.
    ICONS = {
      # window split into two panes
      default: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M3.75 6.75A1.5 1.5 0 0 1 5.25 5.25h13.5a1.5 1.5 0 0 1 1.5 1.5v10.5a1.5 1.5 0 0 1-1.5 1.5H5.25a1.5 1.5 0 0 1-1.5-1.5V6.75z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M9.75 5.25v13.5"/>',
      # bulleted rows
      list: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8.25 6.75h12M8.25 12h12M8.25 17.25h12M3.75 6.75h.008v.008H3.75V6.75zM3.75 12h.008v.008H3.75V12zM3.75 17.25h.008v.008H3.75v-.008z"/>',
      # three vertical columns
      board: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M3.75 5.25h4.5v13.5h-4.5zM9.75 5.25h4.5v13.5h-4.5zM15.75 5.25h4.5v13.5h-4.5z"/>'
    }.freeze

    # board: false drops the Board segment regardless of the feature flag — used on
    # the standalone inbox index, which has no board pane to switch into (a stale
    # saved "board" then falls back to Default via the Stimulus controller).
    def initialize(board: true, **attrs)
      @board = board
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      div(
        # `inbox-view-switcher` is a CSS hook: in List/Board mode the full-width
        # header's right edge sits under the bottom-right reading drawer on short
        # viewports, which would bury this control and trap the user. application.css
        # reorders it to the left of the header there so switching back is reachable.
        class: class_names("inbox-view-switcher inline-flex items-center gap-0.5 rounded-lg bg-muted p-0.5", custom_class),
        role: "group",
        aria: { label: t(".aria_label") },
        **@attrs
      ) do
        enabled_layouts.each { |layout| segment(layout) }
      end
    end

    private

    # Board ships gated off by default until it's production-ready
    # (Features.email_board?); when off, only Default + List are offered. The
    # inbox-layout Stimulus controller derives its valid set from the rendered
    # buttons, so dropping the segment here is enough — a stale saved "board"
    # falls back to Default.
    def enabled_layouts
      (@board && Features.email_board?) ? LAYOUTS : LAYOUTS - %i[board]
    end

    def segment(layout)
      button(
        type: "button",
        class: "inline-flex items-center justify-center rounded-md p-1.5 transition-colors cursor-pointer text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-500",
        title: t(".#{layout}"),
        aria: { label: t(".#{layout}"), pressed: "false" },
        data: {
          inbox_layout_target: "button",
          layout: layout,
          action: "click->inbox-layout#select"
        }
      ) do
        svg(class: "w-4 h-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
          raw(safe(ICONS[layout]))
        end
      end
    end
  end
end
