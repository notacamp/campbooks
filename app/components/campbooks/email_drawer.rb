# frozen_string_literal: true

module Campbooks
  # A compact email reader that floats in the bottom-right corner on desktop and
  # slides up as a bottom sheet on mobile. Opened by the `email-drawer` Stimulus
  # controller, which intercepts /email_messages/:id link clicks and lazy-loads
  # the message into the inner turbo-frame. Used by the List and Board inbox
  # layouts and by email links elsewhere (home feed, search results).
  class EmailDrawer < Campbooks::Base
    def initialize(open: false, size: :lg, **attrs)
      @open = open
      @size = size
      @attrs = attrs
    end

    def with_header(&block)
      @header = block
    end

    def with_body(&block)
      @body = block
    end

    def view_template(&block)
      yield(self) if block

      div(data: { controller: "email-drawer" }, **@attrs) do
        # Backdrop: dims and closes on tap on mobile (the bottom sheet is modal);
        # absent on desktop so the floating card leaves the list interactive.
        div(
          class: "fixed inset-0 z-30 bg-black/40 sm:hidden",
          data: { email_drawer_target: "backdrop", action: "click->email-drawer#close" },
          style: "display: none"
        )

        # Panel: a full-width bottom sheet on mobile, a floating bottom-right card
        # on desktop. Slides up from below (translate-y) when opened.
        div(
          class: class_names(
            "fixed z-40 bg-card shadow-2xl border border-gray-200 dark:border-gray-700",
            "flex flex-col transition-transform duration-300 ease-out",
            "inset-x-0 bottom-0 max-h-[85vh] rounded-t-2xl",
            "sm:inset-x-auto sm:bottom-4 sm:right-4 sm:rounded-2xl",
            SIZE_CLASSES[@size],
            # When closed, slide off-screen AND hide entirely: translate-y-full
            # alone leaves a ~1rem sliver of the empty card peeking above the
            # viewport bottom on desktop (the panel floats at sm:bottom-4). The
            # email-drawer controller drops `invisible` to reveal+animate it open.
            @open ? "translate-y-0" : "translate-y-full invisible"
          ),
          role: "dialog",
          aria_modal: "true",
          data: { email_drawer_target: "panel", controller: "drawer-resize" }
        ) do
          resize_handles

          if @header
            # pl-7 (not px-4) so the header's leading icon clears the top-left
            # resize grabber that floats over this corner.
            div(class: "pl-7 pr-4 py-2.5 border-b border-gray-200 flex items-center justify-between flex-shrink-0", &@header)
          end

          if @body
            div(class: "flex-1 overflow-y-auto", &@body)
          end
        end
      end
    end

    private

    # Drag-to-resize affordances (desktop only — the panel is a full-width bottom
    # sheet on mobile, where these are hidden). Anchored bottom-right, so the
    # resizable edges are TOP (height) and LEFT (width), with a top-left corner for
    # both. Wired to the drawer-resize Stimulus controller on the panel.
    def resize_handles
      div(class: "hidden sm:block absolute -top-0.5 left-3 right-3 h-1.5 cursor-ns-resize z-20 rounded-full hover:bg-accent-400/40 transition-colors",
          data: { action: "mousedown->drawer-resize#start", drawer_resize_axis_param: "y" })
      div(class: "hidden sm:block absolute -left-0.5 top-3 bottom-3 w-1.5 cursor-ew-resize z-20 rounded-full hover:bg-accent-400/40 transition-colors",
          data: { action: "mousedown->drawer-resize#start", drawer_resize_axis_param: "x" })
      div(class: "hidden sm:flex absolute top-0 left-0 w-7 h-7 cursor-nwse-resize z-30 group items-start justify-start",
          data: { action: "mousedown->drawer-resize#start", drawer_resize_axis_param: "xy" },
          title: t("components.resize_handle.hint")) do
        # Always-visible grabber: a small bordered card holding a 2×2 dot grid so
        # the resize corner reads as draggable. Border + dots darken on hover.
        # Mirrors the Campbooks::ResizeHandle pill grabber on the inbox panes.
        div(class: "pointer-events-none m-1.5 flex items-center justify-center w-4 h-4 rounded-md bg-card border border-border shadow-sm transition-colors group-hover:border-gray-300 dark:group-hover:border-gray-600") do
          svg(class: "w-2 h-2 text-gray-400 dark:text-gray-500 group-hover:text-gray-700 dark:group-hover:text-gray-200 transition-colors",
              viewBox: "0 0 4 4", fill: "currentColor", aria_hidden: "true") do
            raw(safe('<circle cx="1" cy="1" r="0.7"/><circle cx="3" cy="1" r="0.7"/><circle cx="1" cy="3" r="0.7"/><circle cx="3" cy="3" r="0.7"/>'))
          end
        end
      end
    end

    # Mobile is always a full-width bottom sheet (inset-x-0); these set the
    # desktop floating-card starting width + height. Height is capped to the
    # viewport (matching the drawer-resize JS clamp) so it never overflows.
    SIZE_CLASSES = {
      md: "sm:w-96 sm:h-[30rem] sm:max-h-[calc(100dvh_-_2rem)]",
      lg: "sm:w-[440px] sm:h-[560px] sm:max-h-[calc(100dvh_-_2rem)]",
      xl: "sm:w-[520px] sm:h-[640px] sm:max-h-[calc(100dvh_-_2rem)]"
    }.freeze
  end
end
