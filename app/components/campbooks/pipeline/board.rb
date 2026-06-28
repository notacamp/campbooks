# frozen_string_literal: true

module Campbooks
  module Pipeline
    # The pipeline kanban board body (rendered inside the pipeline_board Turbo
    # Frame): a header with a back link, the pipeline name and an "Add items"
    # button, then the horizontally-scrolling stage columns. The board is wrapped
    # in the pipeline-board Stimulus controller that drives drag-and-drop.
    class Board < Campbooks::Base
      def initialize(pipeline:, columns:)
        @pipeline = pipeline
        @columns = columns
      end

      def view_template
        div(
          class: "flex flex-col rounded-xl border border-border bg-card overflow-hidden",
          data: { controller: "pipeline-board", pipeline_board_move_url_value: helpers.move_pipeline_path(@pipeline) }
        ) do
          header
          if @columns.empty?
            no_stages
          else
            columns
          end
        end
      end

      private

      def header
        div(class: "flex items-center justify-between gap-3 px-4 py-3 border-b border-border flex-shrink-0") do
          div(class: "flex items-center gap-3 min-w-0") do
            a(href: helpers.settings_pipelines_path, class: "text-muted-foreground hover:text-foreground flex-shrink-0", aria_label: t(".back")) do
              svg(class: "h-5 w-5", fill: "none", stroke: "currentColor", stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
                raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M15.75 19.5L8.25 12l7.5-7.5"/>'))
              end
            end
            h2(class: "text-sm font-semibold text-foreground truncate") { @pipeline.name }
          end
          div(class: "flex items-center gap-3 flex-shrink-0") do
            span(class: "text-[11px] text-muted-foreground hidden sm:inline") { t(".drag_hint") }
            render(Campbooks::Button.new(
              variant: :primary, size: :sm,
              href: helpers.new_pipeline_membership_path(@pipeline),
              data: { turbo_frame: "pipeline_picker" }
            )) { t(".add_items") }
          end
        end
      end

      def columns
        div(class: "overflow-x-auto overflow-y-hidden p-4") do
          div(class: "flex gap-4 min-w-max h-[calc(100dvh-14rem)] min-h-[22rem]") do
            @columns.each { |column| render(Campbooks::Pipeline::BoardColumn.new(column: column)) }
          end
        end
      end

      def no_stages
        div(class: "flex items-center justify-center p-8 min-h-[22rem]") do
          render(Campbooks::EmptyState.new(variant: :standalone, title: t(".no_stages_title"), description: t(".no_stages_description"))) do |es|
            es.with_actions do
              render(Campbooks::Button.new(variant: :outline, size: :sm, href: helpers.edit_settings_pipeline_path(@pipeline))) { t(".add_stages") }
            end
          end
        end
      end
    end
  end
end
