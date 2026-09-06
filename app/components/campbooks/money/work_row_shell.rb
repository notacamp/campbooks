# frozen_string_literal: true

module Campbooks
  module Money
    # The one row shape every "Needs you" item shares: an Ember dot, a title, a
    # meta line, and right-aligned actions that drop under the text on a phone.
    # Callers wrap it (a block wrapper lets the hunt panel or a form land below).
    module WorkRowShell
      ROW_CLASSES     = "flex flex-wrap items-start gap-3 rounded-xl px-4 py-3.5 transition-colors hover:bg-muted/50 sm:flex-nowrap sm:px-6"
      MAIN_CLASSES    = "min-w-0 flex-1 basis-[calc(100%-1.25rem)] sm:basis-auto"
      ACTIONS_CLASSES = "flex w-full flex-wrap items-center justify-end gap-2 pl-5 sm:w-auto sm:shrink-0 sm:pl-0"
      WRAPPER_CLASSES = "-mx-4 sm:-mx-6"

      def work_row(title:, meta: [], &actions)
        div(class: ROW_CLASSES) do
          span(class: "mt-[6px] h-2 w-2 shrink-0 rounded-full bg-ember-gradient shadow-ember-glow", aria_hidden: "true")
          div(class: MAIN_CLASSES) do
            div(class: "text-[14px] font-semibold leading-snug text-foreground") { plain title }
            work_meta(meta) if meta.present?
          end
          div(class: ACTIONS_CLASSES, &actions) if actions
        end
      end

      # Meta parts are strings, or { nif: true } for the warning flag.
      def work_meta(parts)
        div(class: "mt-0.5 flex flex-wrap items-center gap-x-1.5 gap-y-0.5 text-[12.5px] text-muted-foreground") do
          parts.each_with_index do |part, i|
            span(class: "mx-0.5 opacity-40", aria_hidden: "true") { plain "·" } if i.positive?
            if part.is_a?(Hash) && part[:nif]
              span(class: "rounded border border-warning/40 px-1 text-[10px] font-bold text-warning") { plain "NIF" }
            else
              span { plain part.to_s }
            end
          end
        end
      end

      def post_form(action, hidden: {}, method: :post, confirm: nil, css: "inline-flex")
        data = confirm ? { turbo_confirm: confirm } : {}
        form(action: action, method: :post, class: css, data: data) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: method.to_s) unless method == :post
          hidden.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          yield
        end
      end

      def button_classes(variant, size: :sm)
        class_names(Campbooks::Button::BASE_CLASSES, Campbooks::Button::VARIANT_CLASSES[variant], Campbooks::Button::SIZE_CLASSES[size])
      end
    end
  end
end
