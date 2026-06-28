# frozen_string_literal: true

module Campbooks
  module Pipeline
    # One stage column on a pipeline board: a header with the item count in the
    # stage's colour plus a scrollable, droppable list of item cards. Terminal
    # stages are read-only drop targets (cards can be dropped IN but not dragged
    # out) — marked data-droppable="false" with a lock hint.
    class BoardColumn < Campbooks::Base
      def initialize(column:)
        @stage = column[:stage]
        @memberships = column[:memberships] || []
        @has_more = column[:has_more]
        @draggable = column[:draggable]
        @pipeline = @stage.pipeline
      end

      def view_template
        div(class: "flex w-72 sm:w-80 flex-shrink-0 flex-col rounded-xl border border-border bg-muted/40 max-h-full") do
          header
          div(
            class: "flex-1 space-y-2 overflow-y-auto p-2 min-h-[4rem]",
            data: {
              action: "dragover->pipeline-board#dragOver dragleave->pipeline-board#dragLeave drop->pipeline-board#drop",
              pipeline_board_target: "dropzone",
              stage_id: @stage.id,
              droppable: @draggable.to_s
            }
          ) do
            if @memberships.empty?
              p(class: "px-2 py-6 text-center text-[11px] text-muted-foreground") { t(".empty") }
            else
              @memberships.each do |membership|
                render(Campbooks::Pipeline::BoardCard.new(membership: membership, pipeline: @pipeline, draggable: @draggable))
              end
              p(class: "px-2 py-1.5 text-center text-[11px] text-muted-foreground") { t(".more") } if @has_more
            end
          end
        end
      end

      private

      def header
        div(class: "flex items-center gap-2 border-b border-border px-3 py-2.5 flex-shrink-0") do
          span(
            class: "inline-flex h-5 min-w-[1.25rem] items-center justify-center rounded-full px-1.5 text-[11px] font-semibold tabular-nums",
            style: "background-color: #{@stage.color}20; color: #{@stage.color}"
          ) { count_label }
          span(class: "text-[13px] font-semibold text-foreground truncate") { @stage.name }
          terminal_badge if @stage.is_terminal?
        end
      end

      def terminal_badge
        span(class: "ml-auto inline-flex items-center", aria_label: t(".terminal_hint")) do
          svg(class: "h-3.5 w-3.5 text-muted-foreground", fill: "none", stroke: "currentColor", stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
            raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>'))
          end
        end
      end

      def count_label
        @has_more ? "#{@memberships.size}+" : @memberships.size.to_s
      end
    end
  end
end
