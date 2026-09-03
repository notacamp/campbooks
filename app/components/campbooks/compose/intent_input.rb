# frozen_string_literal: true

module Campbooks
  module Compose
    # The bold composer's opening move: one line where you tell Scout what to say
    # (or just start typing the message). A light Ember-glass row with a spark, a
    # single big input, and a Draft button.
    #
    # Enter or Draft posts the note to the hidden compose-chat thread, whose
    # auto-actions fill the editor and the envelope below. A long note left
    # untouched (2+ sentences) becomes the message itself the moment the writer
    # clicks into the body — the second of the Rethink's "both produce the draft
    # below" paths. Behaviour lives in the `compose-intent` Stimulus controller.
    #
    # @param intent [String] text to pre-fill (e.g. an overlay "write to X about…").
    class IntentInput < Campbooks::Base
      SPARK_SVG = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[18px] w-[18px]" aria-hidden="true">' \
                  '<path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'

      def initialize(intent: "")
        @intent = intent.to_s
      end

      def view_template
        div(
          class: "scout-glass flex items-center gap-3 rounded-2xl px-3.5 py-2.5 sm:px-4",
          data: { controller: "compose-intent", action: "compose-chat:body-set@window->compose-intent#restore" }
        ) do
          spark
          input(
            type: "text",
            value: @intent,
            autocomplete: "off",
            placeholder: t(".placeholder"),
            aria_label: t(".placeholder"),
            data: {
              compose_intent_target: "input",
              action: "keydown->compose-intent#keydown input->compose-intent#localInput blur->compose-intent#maybeMoveToBody"
            },
            # focus:shadow-none beats the global [type=text]:focus ring — the glass
            # row (not the control) is the surface here.
            class: "min-w-0 flex-1 border-none bg-transparent p-0 text-[17px] leading-snug text-foreground " \
                   "placeholder:text-muted-foreground focus:outline-none focus:shadow-none"
          )
          draft_button
        end
      end

      private

      def spark
        span(class: "flex h-6 w-6 flex-shrink-0 items-center justify-center", style: "color: var(--ember-solid)", aria_hidden: "true") do
          raw(safe(SPARK_SVG))
        end
      end

      def draft_button
        button(
          type: "button",
          data: { compose_intent_target: "draftButton", action: "click->compose-intent#draft" },
          class: "inline-flex h-8 flex-shrink-0 items-center justify-center gap-1.5 rounded-xl bg-primary px-3.5 " \
                 "text-[13px] font-semibold text-primary-foreground transition hover:opacity-90 disabled:opacity-60"
        ) do
          span(class: "hidden", data: { compose_intent_target: "spinner" }, aria_hidden: "true") do
            svg(class: "h-3.5 w-3.5 animate-spin", fill: "none", viewBox: "0 0 24 24") do
              raw(safe('<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>' \
                       '<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>'))
            end
          end
          span(data: { compose_intent_target: "label" }) { t(".draft") }
        end
      end
    end
  end
end
