# frozen_string_literal: true

module Campbooks
  module Paper
    # One document as a row of facts: the file + its title, the "Scout read" (facts, amount
    # bold), the derived status chip, when it landed, and the kebab. Renders as a table row
    # (`layout: :table`, desktop) or a stacked card (`layout: :card`, mobile) — the same
    # component, so DocumentsController#settle can refresh both from one place.
    class Row < Campbooks::Base
      FILE_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/></svg>'

      def initialize(document:, folders: [], layout: :table)
        @doc = document
        @folders = folders || []
        @layout = layout
      end

      def view_template
        @layout == :card ? card : table_row
      end

      private

      def status
        @status ||= Documents::Status.for(@doc)
      end

      def facts
        @facts ||= Documents::Facts.for(@doc)
      end

      # ── Desktop table row ────────────────────────────────────────────────────
      def table_row
        tr(id: helpers.dom_id(@doc, :paper_row), class: "group align-middle") do
          td(class: cell("py-3 pl-3 pr-2")) { document_cell }
          td(class: cell("px-2 py-3")) { scout_read }
          td(class: cell("px-2 py-3")) { render_status_chip }
          td(class: cell("px-2 py-3 text-[12.5px] tabular-nums text-muted-foreground")) { added_label }
          td(class: cell("py-3 pl-2 pr-3 text-right")) { render Campbooks::Paper::RowMenu.new(document: @doc, folders: @folders) }
        end
      end

      # Shared td classes: hairline under each row + a hover fill that rounds the row ends.
      def cell(extra)
        class_names("border-b border-border/60 group-hover:bg-secondary/70", extra)
      end

      # ── Mobile card ──────────────────────────────────────────────────────────
      def card
        div(id: helpers.dom_id(@doc, :paper_card), class: "flex items-start gap-3 border-b border-border/60 py-3.5") do
          file_icon
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-start justify-between gap-2") do
              title_link(css: "text-[14px] font-medium text-foreground")
              render Campbooks::Paper::RowMenu.new(document: @doc, folders: @folders)
            end
            div(class: "mt-1 text-[13px] text-muted-foreground") { scout_read }
            div(class: "mt-2 flex flex-wrap items-center gap-2") do
              render_status_chip
              span(class: "text-[12px] tabular-nums text-muted-foreground") { added_label }
            end
          end
        end
      end

      # ── Pieces ───────────────────────────────────────────────────────────────
      def document_cell
        div(class: "flex items-center gap-2.5") do
          file_icon
          title_link(css: "truncate text-[13.5px] font-medium text-foreground")
        end
      end

      def file_icon
        span(class: "inline-flex h-[30px] w-[30px] flex-shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground") do
          raw(safe(FILE_ICON))
        end
      end

      def title_link(css:)
        a(href: helpers.document_path(@doc), class: css, data: { turbo_frame: "_top" }) { @doc.display_title }
      end

      def scout_read
        span(class: "text-[13.5px] text-foreground/90") do
          facts.segments.each_with_index do |segment, index|
            span(class: "text-muted-foreground") { " · " } if index.positive?
            if segment.emphasis
              span(class: "font-semibold tabular-nums text-foreground") { segment.text }
            else
              plain(segment.text)
            end
          end
        end
      end

      def render_status_chip
        render Campbooks::StatusChip.for(status)
      end

      def added_label
        date = @doc.created_at.to_date
        date == Date.current ? t("components.paper.row.today") : l(date, format: :day_month)
      end
    end
  end
end
