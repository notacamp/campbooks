# frozen_string_literal: true

module Campbooks
  module Memory
    # The "Teach Scout something" row at the bottom of Scout's memory — a light
    # Ember glass bar with a spark, a free-text input and a Teach button. It's the
    # same input as the Scout bar: a sentence posts to settings/memory/teach and
    # comes back as a new memory row (or an inline explanation when Scout can't
    # learn it yet). Re-rendered by the teach turbo-stream, so it also carries the
    # last response message.
    class TeachBox < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-[18px] w-[18px]" aria-hidden="true">' \
              '<path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'

      # @param message [String, nil] last teach response (an "I can't learn that" note)
      # @param error [Boolean] whether the message is an error/refusal (vs neutral)
      def initialize(message: nil, error: false)
        @message = message
        @error = error
      end

      def view_template
        div(id: "memory_teach", class: "mt-4") do
          form(
            action: helpers.settings_memory_teach_path, method: "post",
            class: "flex items-center gap-2.5 rounded-2xl border border-border p-2 pl-3.5",
            style: "background-color: color-mix(in oklch, var(--ember-solid) 6%, var(--card))"
          ) do
            input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
            span(class: "flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(SPARK)) }
            input(
              type: "text", name: "sentence", autocomplete: "off",
              placeholder: t(".placeholder"),
              "aria-label": t(".placeholder"),
              class: "min-w-0 flex-1 border-0 bg-transparent p-0 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0"
            )
            button(
              type: "submit",
              class: "flex-shrink-0 rounded-lg bg-primary px-3.5 py-1.5 text-sm font-medium text-primary-foreground transition hover:bg-primary/90"
            ) { t(".button") }
          end
          message_row if @message.present?
        end
      end

      private

      def message_row
        p(class: class_names("mt-2 px-1 text-[13px]", @error ? "text-muted-foreground" : "text-muted-foreground")) do
          @message
        end
      end
    end
  end
end
