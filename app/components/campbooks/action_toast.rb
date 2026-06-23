# frozen_string_literal: true

module Campbooks
  # The app's single action-feedback snackbar. A frosted capsule with a small
  # colored icon badge per variant, a message, and an optional *distinct* Undo
  # button — visually identical to the Skim / sync Campbooks::StatusFeedback pill
  # so feedback feels the same everywhere.
  #
  # Appended into the centered #action_toasts region (see shared/_flash_toast_region)
  # and auto-dismissed by the `action-toast` Stimulus controller after `duration`.
  #
  #   # plain feedback
  #   notify_stream("Saved")                       # → ActionToast(variant: :success)
  #
  #   # reversible action: pass `undo:` and the capsule grows a distinct Undo button
  #   # whose form POSTs `params` to `endpoint` (a Turbo Stream) to reverse it.
  #   Campbooks::ActionToast.new(
  #     message: "Archived 3 emails", variant: :success,
  #     undo: { endpoint: bulk_path, params: { "tool" => "unarchive", "email_ids[]" => ids } }
  #   )
  class ActionToast < Campbooks::Base
    # DOM id of the region toasts are appended into (see shared/_flash_toast_region).
    REGION_ID = "action_toasts"

    # Frosted capsule shell — mirrors Campbooks::StatusFeedback::PILL_BASE so the
    # action snackbar and the skim/sync pill stay visually identical.
    CAPSULE = "pointer-events-auto inline-flex max-w-full items-center gap-2.5 rounded-full " \
              "border border-border bg-card/95 py-1.5 pl-2.5 pr-3 text-sm font-medium " \
              "text-foreground shadow-lg backdrop-blur animate-fade-in"

    # Subtle per-variant icon badge: a small tinted circle around the glyph. The
    # capsule itself stays neutral — "a bit of color", not a saturated box.
    BADGE_CLASSES = {
      success: "bg-green-100 text-green-700 dark:bg-green-500/15 dark:text-green-300",
      error:   "bg-red-100 text-red-700 dark:bg-red-500/15 dark:text-red-300",
      warning: "bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300",
      info:    "bg-blue-100 text-blue-700 dark:bg-blue-500/15 dark:text-blue-300"
    }.freeze

    ICONS = {
      success: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>',
      error:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>',
      warning: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-2.694-.833-3.464 0L3.34 16.5c-.77.833.192 2.5 1.732 2.5z"/>',
      info:    '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M12 2a10 10 0 100 20 10 10 0 000-20z"/>'
    }.freeze

    # Distinct accent Undo button — same shape as the skim pill's action button.
    UNDO_CLASSES = "-my-0.5 flex-shrink-0 cursor-pointer rounded-full px-2.5 py-1 text-sm font-semibold " \
                   "text-primary transition-colors hover:bg-primary/10 focus-visible:outline " \
                   "focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent-400"

    # @param message [String]
    # @param variant [Symbol] :success, :error, :warning, :info
    # @param undo [Hash, nil] { endpoint:, params: {}, label: } — renders the Undo
    #   button. `params` values may be arrays (emits repeated inputs, e.g. email_ids[]).
    # @param duration [Integer, nil] ms before auto-dismiss (default: 7000 with undo, 4000 without)
    def initialize(message:, variant: :info, undo: nil, duration: nil, **attrs)
      @message = message
      @variant = variant
      @undo = undo
      @duration = duration || (undo ? 7000 : 4000)
      @attrs = attrs
    end

    def view_template
      div(
        class: CAPSULE,
        role: "status",
        aria_live: "polite",
        data: { action_toast_duration: @duration },
        **@attrs
      ) do
        badge
        span(class: "min-w-0") { @message }
        undo_button if @undo
      end
    end

    private

    def badge
      span(class: class_names(
        "inline-flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full",
        BADGE_CLASSES.fetch(@variant, BADGE_CLASSES[:info])
      )) do
        svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
          raw(safe(ICONS.fetch(@variant, ICONS[:info])))
        end
      end
    end

    # Server-undo: a form (display:contents, so the button is a flex item of the
    # capsule) that POSTs the reverse action as a Turbo Stream.
    def undo_button
      form(action: @undo[:endpoint], method: :post, class: "contents", data: { turbo_stream: true }) do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        (@undo[:params] || {}).each do |key, value|
          Array(value).each { |v| input(type: "hidden", name: key.to_s, value: v.to_s) }
        end
        button(type: "submit", class: UNDO_CLASSES) { @undo[:label] || t(".undo") }
      end
    end
  end
end
