# frozen_string_literal: true

module Campbooks
  module Calendar
    # Modal host for creating/editing a calendar event without leaving the
    # calendar. A native <dialog> wrapping a Turbo Frame that lazy-loads the
    # new/edit form — mirrors the setup-modal pattern (app/views/shared/_setup_modal).
    #
    # Mounted once on the calendar index. Opened by the `calendar-event-modal`
    # Stimulus controller from any element carrying
    # `data-calendar-event-modal-open="<url>"`, from the `calendar-event-modal:open`
    # window event (grid click/drag + the `c` keyboard shortcut), or from a
    # `?new_event=1` deep-link (Cmd+K "New calendar event"). On a successful save
    # the controller breaks out of the frame via a Turbo Stream (see
    # turbo_visit_controller); a validation error re-renders the frame in place so
    # errors show inside the modal.
    class EventModal < Campbooks::Base
      def initialize(open: false, **attrs)
        @open = open
        @attrs = attrs
      end

      def view_template
        div(data: { controller: "calendar-event-modal" }) do
          dialog(
            class: "rounded-2xl shadow-2xl border border-border p-0 overflow-hidden backdrop:bg-black/40 m-auto w-[calc(100vw-2rem)] max-w-lg max-h-[90dvh]",
            aria: { label: t(".aria_label") },
            data: { calendar_event_modal_target: "dialog" },
            open: @open ? "" : nil,
            **@attrs
          ) do
            raw(helpers.turbo_frame_tag(
              "calendar_event_modal",
              class: "block overflow-y-auto max-h-[90dvh]",
              data: { calendar_event_modal_target: "frame" }
            ) do
              helpers.content_tag(:div, t("shared.actions.loading"),
                class: "flex items-center justify-center py-16 text-sm text-muted-foreground")
            end)
          end
        end
      end
    end
  end
end
