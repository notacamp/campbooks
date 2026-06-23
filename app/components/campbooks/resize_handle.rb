module Campbooks
  # Drag-to-resize divider for a horizontally-resizable docked pane (the inbox
  # thread list, the Discussion panel). It drives the `panel-resize` Stimulus
  # controller on the parent pane and, unlike a bare hover-strip, carries an
  # *always-visible* pill grabber — a white, bordered capsule holding grip dots,
  # centered on the divider — so the edge plainly reads as draggable at rest.
  #
  # `edge` says which edge of the pane the handle sits on, which also fixes the
  # drag direction the controller reads:
  #   :right — handle on the RIGHT edge of a left-docked pane (the thread list)
  #   :left  — handle on the LEFT edge of a right-docked pane (Discussion panel)
  #
  # Desktop only: the panes collapse to a single column below `breakpoint` (lg
  # by default), so the handle is hidden there. Extra `data:` attributes are
  # merged onto the element (e.g. a second `chat-panel` target on the Discussion
  # panel handle).
  class ResizeHandle < Campbooks::Base
    EDGES = {
      right: { side: "right-0 -mr-1", round: "rounded-l" },
      left: { side: "left-0 -ml-1", round: "rounded-r" }
    }.freeze

    # A vertical column of 3 dots — the universal "drag me" grip. The default
    # preserveAspectRatio (meet) keeps them round whatever box they scale into.
    GRIP_DOTS = '<circle cx="1.5" cy="2" r="1"/>' \
                '<circle cx="1.5" cy="6" r="1"/>' \
                '<circle cx="1.5" cy="10" r="1"/>'

    def initialize(edge: :right, breakpoint: "lg", data: {}, **attrs)
      @edge = edge.to_sym
      @breakpoint = breakpoint
      @data = data
      @attrs = attrs
    end

    def view_template
      cfg = EDGES.fetch(@edge)
      hint = t(".hint")

      div(
        class: class_names(
          "group absolute top-0 bottom-0 w-2 cursor-col-resize transition-colors",
          # Above every other sidebar element (skim overlay z-40, tag dropdowns
          # z-50) so the protruding pill is never clipped or covered; stays under
          # the global modal layer (z-[60]).
          "z-[55]",
          # Desktop-only by default (panes collapse below the breakpoint); pass
          # breakpoint: nil to always show it (e.g. Lookbook previews).
          (@breakpoint ? "hidden #{@breakpoint}:block" : "block"),
          cfg[:side],
          cfg[:round],
          # Subtle neutral track on hover so the whole hit-zone lights up — never
          # the status-blue this replaced (blue means state, not affordance).
          "hover:bg-accent-400/30",
          @attrs[:class]
        ),
        role: "separator",
        "aria-orientation": "vertical",
        "aria-label": hint,
        title: hint,
        data: {
          action: "mousedown->panel-resize#start",
          panel_resize_target: "handle"
        }.merge(@data)
      ) do
        grip
      end
    end

    private

    # The always-visible affordance: a white (theme card), bordered pill centered
    # on the divider, holding the grip dots. Border + dots darken on hover; never
    # eats the drag's mousedown (pointer-events-none).
    def grip
      div(class: class_names(
        "pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2",
        "flex items-center justify-center w-3.5 h-8 rounded-full",
        "bg-card border border-border shadow-sm transition-colors",
        "group-hover:border-gray-300 dark:group-hover:border-gray-600"
      )) do
        svg(
          class: class_names(
            "w-1 h-4 transition-colors",
            "text-gray-400 dark:text-gray-500",
            "group-hover:text-gray-700 dark:group-hover:text-gray-200"
          ),
          viewBox: "0 0 3 12",
          fill: "currentColor",
          "aria-hidden": "true"
        ) { raw(safe(GRIP_DOTS)) }
      end
    end
  end
end
