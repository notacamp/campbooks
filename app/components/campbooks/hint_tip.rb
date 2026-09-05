# frozen_string_literal: true

module Campbooks
  # The shared shortcut-hint tooltip: rendered once per layout, driven by
  # hint_controller.js. It is purely visual (aria-hidden) — the accessible name
  # lives on the control (aria-label / text) and the key in aria-keyshortcuts.
  #
  # Live use (in layouts):
  #   render Campbooks::HintTip.new
  #
  # Lookbook preview (static, open):
  #   render Campbooks::HintTip.new(preview: { label: "Mark done", keys: %w[D] })
  class HintTip < Campbooks::Base
    # @param preview [Hash, nil] { label: String, keys: Array<String> }
    #   When present, renders the tooltip in a static open state (for Lookbook
    #   previews). When nil (default), renders the live hidden element used by
    #   hint_controller.js.
    def initialize(preview: nil)
      @preview = preview
    end

    def view_template
      if @preview
        preview_tip
      else
        live_tip
      end
    end

    private

    # Live tooltip: hidden by default; hint_controller.js writes content and
    # toggles .is-on. The popover="manual" API places it in the top layer so it
    # rides above dialogs without a stacking-context fight.
    def live_tip
      div(
        id: "hint-tip",
        class: "hint-tip",
        popover: "manual",
        aria: { hidden: "true" },
        data: { hint_target: "tip" }
      ) do
        span(class: "hint-tip-label", data: { hint_target: "label" })
        span(class: "hint-tip-keys", data: { hint_target: "keys" })
      end
    end

    # Preview tooltip: open and filled (for Lookbook). No popover attribute so
    # it stays in normal flow and displays statically.
    def preview_tip
      label_text = @preview[:label].to_s
      keys = Array(@preview[:keys])

      div(class: "hint-tip is-on is-preview", aria: { hidden: "true" }) do
        span(class: "hint-tip-label") { plain label_text }
        if keys.any?
          span(class: "hint-tip-keys") do
            keys.each do |k|
              kbd(class: "hint-tip-kbd") { plain k.to_s }
            end
          end
        end
      end
    end
  end
end
