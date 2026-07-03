# frozen_string_literal: true

module Campbooks
  module Compose
    # Scout's ghost draft inside the composer (Probe 02): an Ember-glass block
    # holding a generated reply until the user takes ownership. "Use this
    # draft" folds the text into the canvas (plain ink, editable); tone chips
    # regenerate with an instruction; "Start blank" dismisses. Until accepted,
    # AI material stays visibly Scout's.
    class ScoutDraft < Campbooks::Base
      TONES = %w[shorter warmer firmer].freeze

      def initialize(text:, message:)
        @text = text.to_s
        @message = message
      end

      def view_template
        div(class: "scout-glass rounded-2xl px-4 py-3.5 mb-3",
            data: { compose_engine_target: "scoutDraft" }) do
          header_row
          div(class: "text-[14px] leading-relaxed text-gray-800 whitespace-pre-line py-2",
              data: { compose_engine_target: "scoutText" }) { @text }
          actions_row
        end
      end

      private

      def header_row
        div(class: "flex items-center gap-2") do
          span(class: "w-5 h-5 rounded-full flex items-center justify-center text-white flex-shrink-0",
               style: "background-image: var(--ember);") do
            svg(class: "w-2.5 h-2.5", fill: "currentColor", viewBox: "0 0 24 24") do
              raw(safe('<path d="M12 2l1.9 6.1L20 10l-6.1 1.9L12 18l-1.9-6.1L4 10l6.1-1.9L12 2z"/>'))
            end
          end
          span(class: "text-[12.5px] font-semibold text-gray-900") { "Scout" }
          span(class: "text-[9px] font-bold tracking-wide uppercase text-gray-500 border border-gray-300/70 rounded px-1 py-px") { t(".ai_tag") }
          span(class: "text-[11px] text-gray-500") { t(".drafted_label") }
        end
      end

      def actions_row
        div(class: "flex items-center gap-2 flex-wrap") do
          button(type: "button",
                 class: "inline-flex items-center px-3 py-1.5 text-xs font-semibold bg-accent-600 text-white rounded-lg hover:bg-accent-700 transition-colors",
                 data: { action: "click->compose-engine#useScoutDraft" }) { t(".use_draft") }
          TONES.each do |tone|
            button(type: "button",
                   class: tone_chip_classes,
                   data: { action: "click->compose-engine#retoneScoutDraft", compose_engine_tone_param: tone }) do
              t(".tone_#{tone}")
            end
          end
          button(type: "button",
                 class: "inline-flex items-center px-2.5 py-1.5 text-xs text-gray-500 hover:text-gray-700 transition-colors",
                 data: { action: "click->compose-engine#dismissScoutDraft" }) { t(".start_blank") }
        end
      end

      def tone_chip_classes
        "inline-flex items-center px-2.5 py-1.5 text-xs font-medium text-gray-600 bg-white/60 dark:bg-white/10 " \
          "border border-gray-200/80 rounded-lg hover:bg-white transition-colors"
      end
    end
  end
end
