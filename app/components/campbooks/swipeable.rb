# frozen_string_literal: true

module Campbooks
  # Wraps any list row/card so it can be swiped/dragged sideways to act, driven by
  # the `swipe-actions` Stimulus controller. Behind the (translating) content it
  # renders one colored action panel per active side; the controller reveals and
  # morphs them as the user drags.
  #
  # Direction is named from the user's point of view:
  #   left:  actions fired by swiping the content LEFT  (reveals the right-anchored panel)
  #   right: actions fired by swiping the content RIGHT (reveals the left-anchored panel)
  #
  # Each is an ordered list of 1–2 stage hashes (stage 1 = short swipe, stage 2 =
  # deep swipe). A stage:
  #   { key:, label:, icon:, color:, endpoint:, method: "post", params: {},
  #     confirm: nil | { title:, message: }, picker: nil | "snooze", removes: true }
  # `icon` is a Symbol from ICONS or a raw inner-SVG string. `color` is one of the
  # keys styled in app/assets/tailwind/application.css. `removes:` (default true)
  # slides the row out on commit (server then removes it); set false for
  # replace-in-place actions (e.g. document approve/reject).
  #
  # Usage from ERB:
  #   <%= render(Campbooks::Swipeable.new(left: [...], right: [...])) do %>
  #     <!-- existing row markup, unchanged -->
  #   <% end %>
  class Swipeable < Campbooks::Base
    # Inner SVG paths (stroke inherits the wrapping <svg stroke-width="2">).
    ICONS = {
      archive: '<path stroke-linecap="round" stroke-linejoin="round" d="M5 8h14M5 8a2 2 0 01-2-2V5a2 2 0 012-2h14a2 2 0 012 2v1a2 2 0 01-2 2M5 8v9a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/>',
      snooze: '<path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      trash: '<path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>',
      delete: '<path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-2.694-.833-3.464 0L3.34 16.5c-.77.833.192 2.5 1.732 2.5z"/>',
      dismiss: '<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>',
      approve: '<path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7"/>',
      reject: '<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>'
    }.freeze

    # @param surface [String] CSS color the content is painted with *while dragging*
    #   so it cleanly covers the panel (transparent at rest — the row looks unchanged).
    def initialize(left: [], right: [], surface: "var(--color-card)", **attrs)
      @left = Array(left).compact
      @right = Array(right).compact
      @surface = surface
      @attrs = attrs
    end

    def view_template(&block)
      caller_class = @attrs.delete(:class)
      caller_style = @attrs.delete(:style)
      caller_data = @attrs.delete(:data) || {}

      div(
        class: class_names("relative overflow-hidden", caller_class),
        style: [ "--swipe-surface:#{@surface}", caller_style ].compact.join(";"),
        data: {
          controller: "swipe-actions",
          swipe_actions_left_value: @left.map { |s| serialize_stage(s) }.to_json,
          swipe_actions_right_value: @right.map { |s| serialize_stage(s) }.to_json,
          swipe_actions_error_message_value: t(".error")
        }.merge(caller_data),
        **@attrs
      ) do
        # The left panel is revealed by a RIGHT swipe (so it shows @right's action);
        # the right panel is revealed by a LEFT swipe (so it shows @left's action).
        panel(:left, @right.first)
        panel(:right, @left.first)

        div(class: "relative z-10", data: { swipe_actions_target: "content" }) do
          yield if block
        end
      end
    end

    private

    def panel(side, cfg)
      return unless cfg

      edge = side == :left ? "left-2.5 justify-start" : "right-2.5 justify-end"
      div(
        class: "absolute inset-y-0 #{edge} z-0 flex items-center pointer-events-none",
        style: "display:none;opacity:0",
        data: { swipe_actions_target: "#{side}Panel", color: cfg[:color] }
      ) do
        # A light accent-tinted round badge with the icon + label in the accent
        # color (the app's toast/badge language), not a solid color fill. The tint,
        # ring and ink all derive from --swipe-accent (so they morph together and
        # adapt to dark mode); the revealed gutter stays clean canvas.
        div(class: "flex flex-col items-center gap-1.5") do
          div(
            class: "flex h-11 w-11 items-center justify-center rounded-full border transition-colors duration-100",
            style: "background:color-mix(in oklch, var(--swipe-accent) 16%, var(--color-card));border-color:color-mix(in oklch, var(--swipe-accent) 28%, transparent);color:var(--swipe-accent)"
          ) do
            svg(
              class: "h-5 w-5", fill: "none", stroke: "currentColor", stroke_width: "1.75",
              viewBox: "0 0 24 24", data: { swipe_actions_target: "#{side}Icon" }
            ) { raw(safe(icon_svg(cfg[:icon]))) }
          end
          span(
            class: "text-[11px] font-semibold tracking-tight transition-colors duration-100",
            style: "color:var(--swipe-accent)",
            data: { swipe_actions_target: "#{side}Label" }
          ) { cfg[:label] }
        end
      end
    end

    def serialize_stage(cfg)
      {
        key: cfg[:key],
        label: cfg[:label],
        iconSvg: icon_svg(cfg[:icon]),
        color: cfg[:color],
        endpoint: cfg[:endpoint],
        method: (cfg[:method] || "post").to_s,
        params: cfg[:params] || {},
        confirm: serialize_confirm(cfg[:confirm]),
        picker: cfg[:picker],
        removes: cfg.fetch(:removes, true)
      }
    end

    # Normalize a confirm gate to the JS shape (camelCase). `remember_key` opts the
    # dialog into a "Don't ask again" checkbox that persists a skip in localStorage.
    def serialize_confirm(confirm)
      return nil unless confirm
      { title: confirm[:title], message: confirm[:message], rememberKey: confirm[:remember_key] }.compact
    end

    def icon_svg(icon)
      return "" if icon.nil?
      icon.is_a?(Symbol) ? ICONS.fetch(icon, "") : icon.to_s
    end
  end
end
