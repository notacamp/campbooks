# frozen_string_literal: true

module Campbooks
  module Now
    # One setup step, shown as a card in the decision deck — so day zero and day
    # thirty are the same page (the Rethink's "setup is a card in the stack"). The
    # card's frame comes from the deck (.now-deck-stack); this renders its content:
    # a title, a line of help, the step's CTA (opening the same setup modal / page
    # the SetupHub uses), and a quiet "Later" that posts the existing dismiss
    # endpoint. `item` is one localized SetupStatus incomplete-item hash.
    class SetupCard < Campbooks::Base
      SPARK_SVG = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[15px] w-[15px]" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'

      def initialize(item:)
        @item = item
      end

      def view_template
        div(data: { now_deck_card: "" }) do
          div(class: "flex items-center gap-2") do
            span(class: "inline-flex items-center gap-1.5 rounded-md bg-secondary px-2 py-0.5 text-[11.5px] font-medium text-muted-foreground") do
              span(style: "color: var(--ember-solid)") { raw(safe(SPARK_SVG)) }
              plain(t(".chip"))
            end
          end

          h3(class: "mt-3 text-[17px] font-semibold leading-snug tracking-[-0.01em] text-foreground") { @item[:message] }
          p(class: "mt-1.5 text-sm leading-relaxed text-muted-foreground") { @item[:description] } if @item[:description].present?

          div(class: "mt-5 flex items-center justify-end gap-2") do
            later_button
            cta
          end
        end
      end

      private

      def cta
        if @item[:cta_modal]
          render Campbooks::Button.new(
            variant: :primary, size: :sm,
            data: { setup_modal_open: helpers.setup_path(@item[:key], return_to: helpers.now_path) }
          ) { @item[:cta_text] }
        else
          render Campbooks::Button.new(variant: :primary, size: :sm, href: @item[:cta_path]) { @item[:cta_text] }
        end
      end

      # Posts the existing setup dismiss endpoint (adds the key to the workspace's
      # dismissed list); its redirect_back lands back on /now, re-rendering without
      # this card.
      def later_button
        raw(safe(helpers.button_to(
          t(".later"),
          helpers.dismiss_setup_path(key: @item[:key]),
          method: :post,
          class: "rounded-lg px-3 py-1.5 text-[13px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground cursor-pointer border-0 bg-transparent",
          form: { class: "inline-flex shrink-0" }
        )))
      end
    end
  end
end
