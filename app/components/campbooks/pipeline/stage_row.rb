# frozen_string_literal: true

module Campbooks
  module Pipeline
    # One editable stage row in the pipeline form. Rendered once per existing
    # stage AND once (with index "NEW_RECORD") inside the <template> the
    # pipeline-builder controller clones when "Add stage" is clicked — so the row
    # markup lives in exactly one place. Field names follow Rails nested
    # attributes: pipeline[stages_attributes][<index>][<attr>].
    class StageRow < Campbooks::Base
      def initialize(index:, stage: nil)
        @index = index # Integer for persisted/seed rows, "NEW_RECORD" in the template
        @stage = stage
      end

      def view_template
        div(
          class: "flex items-start gap-3 rounded-lg border border-border bg-muted/30 p-3",
          draggable: "true",
          data: {
            pipeline_builder_target: "stageRow",
            action: "dragstart->pipeline-builder#dragStart dragover->pipeline-builder#dragOver " \
                    "dragleave->pipeline-builder#dragLeave drop->pipeline-builder#drop dragend->pipeline-builder#dragEnd"
          }
        ) do
          drag_handle
          hidden(:id, @stage&.id)
          input(type: "hidden", name: field(:position), value: position_value, data: { pipeline_builder_target: "position" })
          hidden(:_destroy, "false")

          div(class: "flex-1 grid grid-cols-1 sm:grid-cols-3 gap-2") do
            render(Campbooks::Input.new(field(:name), placeholder: t(".name_placeholder"), value: @stage&.name, required: true, rounded: :md))
            render(Campbooks::Input.new(field(:description), placeholder: t(".description_placeholder"), value: @stage&.description, rounded: :md))
            div(class: "space-y-2") do
              render(Campbooks::ColorSwatchPicker.new(name: field(:color), selected: selected_color, colors: swatches, include_none: false))
              terminal_toggle
            end
          end

          remove_button
        end
      end

      private

      def field(attr) = "pipeline[stages_attributes][#{@index}][#{attr}]"

      def position_value
        return "" if @index == "NEW_RECORD"

        @stage&.position || (@index + 1)
      end

      def selected_color = @stage&.color.presence || Campbooks::Pipeline::StageRow.default_color

      def self.default_color = PipelineStage::PALETTE.first

      # Always offer the stage's own colour even if it predates the palette, so it
      # stays selected on edit.
      def swatches
        (PipelineStage::PALETTE + [ @stage&.color ]).compact.uniq.map { |hex| { hex: hex, name: hex } }
      end

      def hidden(attr, value)
        input(type: "hidden", name: field(attr), value: value)
      end

      def drag_handle
        div(class: "flex-shrink-0 mt-1 cursor-grab text-muted-foreground/40 hover:text-muted-foreground", aria_hidden: "true") do
          svg(class: "h-4 w-4", fill: "currentColor", viewBox: "0 0 24 24") do
            raw(safe('<circle cx="9" cy="5" r="1.5"/><circle cx="15" cy="5" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="19" r="1.5"/><circle cx="15" cy="19" r="1.5"/>'))
          end
        end
      end

      def terminal_toggle
        label(class: "flex items-center gap-1.5 text-[12px] text-muted-foreground cursor-pointer select-none") do
          input(type: "hidden", name: field(:is_terminal), value: "0")
          input(type: "checkbox", name: field(:is_terminal), value: "1", checked: @stage&.is_terminal?, class: "rounded border-border text-accent-600 focus:ring-accent-500")
          plain(t(".terminal"))
        end
      end

      def remove_button
        button(
          type: "button",
          class: "flex-shrink-0 mt-1 p-1 text-muted-foreground/40 hover:text-red-500 transition",
          aria_label: t(".remove_stage"),
          data: { action: "click->pipeline-builder#removeStage" }
        ) do
          svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
            raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>'))
          end
        end
      end
    end
  end
end
