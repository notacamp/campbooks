# frozen_string_literal: true

module Campbooks
  module Paper
    # The Paper list: a facts table on desktop (a horizontal-scroll wrapper at ≥ sm) and a
    # stack of cards on mobile. Both render the same documents through Campbooks::Paper::Row;
    # the tbody (#paper_rows) and card list (#paper_cards) are the append targets for the
    # lazy pagination sentinel.
    class Table < Campbooks::Base
      COLUMNS = %i[document scout_read status added].freeze

      def initialize(documents:, folders: [])
        @documents = documents
        @folders = folders || []
      end

      def view_template
        desktop
        mobile
      end

      private

      def desktop
        div(class: "hidden overflow-x-auto sm:block") do
          table(class: "w-full min-w-[640px] border-collapse text-[13.5px]") do
            thead do
              tr do
                COLUMNS.each { |col| th(class: header_class(col)) { t("components.paper.table.#{col}") } }
                th(class: "border-b border-border px-2 py-2") { span(class: "sr-only") { t("components.paper.table.status") } }
              end
            end
            tbody(id: "paper_rows") do
              @documents.each { |document| render Campbooks::Paper::Row.new(document: document, folders: @folders, layout: :table) }
            end
          end
        end
      end

      def mobile
        div(id: "paper_cards", class: "sm:hidden") do
          @documents.each { |document| render Campbooks::Paper::Row.new(document: document, folders: @folders, layout: :card) }
        end
      end

      def header_class(_column)
        "border-b border-border px-2 py-2 text-left text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
      end
    end
  end
end
