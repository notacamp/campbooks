# frozen_string_literal: true

module Campbooks
  # One column of the inbox status board (Inbox / Snoozed / Awaiting reply /
  # Done). Renders a header with a count and a scrollable, droppable list of
  # BoardCards. The Awaiting column is read-only — it takes no drops (marked
  # data-droppable="false" with a lock hint) though its cards can be dragged out.
  class BoardColumn < Campbooks::Base
    # Reuse the harmonized semantic tones from application.css for the count chip.
    TONES = {
      inbox: "tone-blue", snoozed: "tone-amber", awaiting: "tone-violet", done: "tone-green"
    }.freeze

    def initialize(column:)
      @column = column
      @key = column[:key].to_sym
      @threads = column[:threads] || []
      @draggable = column[:draggable]
    end

    def view_template
      div(
        class: "flex w-72 sm:w-80 flex-shrink-0 flex-col rounded-xl border border-border bg-muted/40 max-h-full",
        data: { inbox_board_target: "column" }
      ) do
        header
        div(
          class: "flex-1 space-y-2 overflow-y-auto p-2 min-h-[4rem]",
          data: { inbox_board_target: "dropzone", column: @key, droppable: @draggable.to_s }
        ) do
          if @threads.empty?
            p(class: "px-2 py-6 text-center text-[11px] text-muted-foreground") { t(".empty") }
          else
            @threads.each { |thread| render(Campbooks::BoardCard.new(thread: thread, column_key: @key, draggable: @draggable)) }
            p(class: "px-2 py-1.5 text-center text-[11px] text-muted-foreground") { t(".more") } if @column[:has_more]
          end
        end
      end
    end

    private

    def header
      div(class: "flex items-center gap-2 border-b border-border px-3 py-2.5 flex-shrink-0") do
        span(class: class_names("inline-flex h-5 min-w-[1.25rem] items-center justify-center rounded-full px-1.5 text-[11px] font-semibold tabular-nums", TONES.fetch(@key, "tone-neutral"))) { count_label }
        span(class: "text-[13px] font-semibold text-foreground") { t(".#{@key}") }
        next if @draggable

        span(class: "ml-auto inline-flex items-center", title: t(".awaiting_hint")) do
          svg(class: "h-3.5 w-3.5 text-muted-foreground", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
            raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 0h10.5a1.5 1.5 0 011.5 1.5v6a1.5 1.5 0 01-1.5 1.5H6.75a1.5 1.5 0 01-1.5-1.5v-6a1.5 1.5 0 011.5-1.5z"/>'))
          end
        end
      end
    end

    def count_label
      @column[:has_more] ? "#{@threads.size}+" : @threads.size.to_s
    end
  end
end
