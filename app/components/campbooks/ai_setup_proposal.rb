# frozen_string_literal: true

module Campbooks
  # The final step of the conversational setup flow: Scout's proposed Document
  # Types / Tags as a selectable, editable list. Everything is checked by
  # default; the user unchecks what they don't want, optionally edits a row, then
  # applies. extraction_schema (document types) rides along server-side from the
  # stored proposal — only name/color/prompt are editable here.
  class AiSetupProposal < Campbooks::Base
    # @param items [Array<Hash>] [{ "name", "color", "prompt", "extraction_schema"? }]
    # @param kind [String, Symbol] "document_types" | "tags"
    # @param form_action [String] POST target that applies the selection
    def initialize(items:, kind:, form_action:)
      @items = Array(items)
      @kind = kind.to_s
      @form_action = form_action
    end

    def view_template
      div(class: "px-4 pb-4 pt-1 animate-fade-in") do
        if @items.empty?
          p(class: "text-sm text-muted-foreground px-0.5") { t(".no_suggestions") }
          return
        end

        form(action: @form_action, method: "post", class: "space-y-3") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)

          p(class: "text-[11px] font-semibold uppercase tracking-wider text-muted-foreground px-0.5") do
            t(".suggested_heading", kind: label_for(@kind))
          end

          div(class: "space-y-2") do
            @items.each_with_index { |item, i| proposal_row(item, i) }
          end

          div(class: "flex items-center gap-2 pt-1") do
            render Campbooks::Button.new(variant: :primary, size: :sm, type: :submit) { t(".add_selected") }
            render Campbooks::Button.new(variant: :ghost, size: :sm, data: { setup_modal_close: true }) { t("shared.actions.skip_for_now") }
          end
        end
      end
    end

    private

    def label_for(kind)
      kind == "document_types" ? t(".document_types_label") : t(".tags_label")
    end

    def proposal_row(item, index)
      base = "items[#{index}]"
      checkbox_id = "ai_setup_item_#{index}"

      div(class: "rounded-lg border border-border p-3 transition-colors has-[:checked]:border-accent-500 has-[:checked]:bg-accent-50/60") do
        div(class: "flex items-start gap-3") do
          input(
            type: "checkbox", id: checkbox_id, name: "#{base}[selected]", value: "1",
            checked: true, class: "mt-1 h-4 w-4 accent-accent-600 cursor-pointer flex-shrink-0"
          )
          div(class: "min-w-0 flex-1") do
            label(for: checkbox_id, class: "flex items-center gap-2 cursor-pointer") do
              render Campbooks::ColorDot.new(color: item["color"].presence || "#6366f1", size: :md)
              span(class: "text-sm font-medium text-foreground") { item["name"].to_s.humanize }
            end
            p(class: "text-xs text-muted-foreground mt-1 line-clamp-2") { item["prompt"] }

            details(class: "mt-1.5 group") do
              summary(class: "text-xs text-accent-600 cursor-pointer select-none inline-flex items-center gap-1") do
                plain t(".edit_label")
              end
              edit_fields(item, base)
            end
          end
        end
      end
    end

    def edit_fields(item, base)
      div(class: "mt-2 space-y-2") do
        field_row(t(".field_name")) do
          input(type: "text", name: "#{base}[name]", value: item["name"], class: text_input_classes)
        end
        field_row(t(".field_colour")) do
          input(type: "color", name: "#{base}[color]", value: item["color"].presence || "#6366f1",
                class: "h-8 w-12 rounded border border-border bg-background p-0.5 cursor-pointer")
        end
        field_row(t(".field_prompt")) do
          textarea(name: "#{base}[prompt]", rows: "2", class: text_input_classes) { item["prompt"] }
        end
      end
    end

    def field_row(label_text, &block)
      div do
        span(class: "block text-[11px] font-medium text-muted-foreground mb-1") { label_text }
        yield
      end
    end

    def text_input_classes
      "block w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm text-foreground " \
        "placeholder:text-muted-foreground focus:border-accent-500 focus:outline-none focus:ring-1 focus:ring-accent-500"
    end
  end
end
