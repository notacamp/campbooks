# frozen_string_literal: true

module Campbooks
  class Table < Campbooks::Base
    # @param id [String, nil] optional id for the table element
    # @param columns [Array<Hash>] column definitions with :key, :label, optional :align, :width
    # @param rows [Array] row objects responding to column keys (public_send or [])
    # @param row_url [Proc, nil] proc receiving a row and returning a URL for clickable rows
    # @param empty_state [String, nil] message shown when rows are empty
    # @yield [row, col] optional block for custom cell rendering (executed in Phlex context)
    def initialize(id: nil, columns:, rows:, row_url: nil, empty_state: nil, **attrs)
      @id = id
      @columns = columns
      @rows = rows
      @row_url = row_url
      @empty_state = empty_state
      @attrs = attrs
    end

    def view_template(&block)
      @cell_block = block

      div(class: "bg-card shadow-sm rounded-lg border border-border overflow-x-auto", **@attrs) do
        table(id: @id, class: "min-w-full divide-y divide-border") do
          thead(class: "bg-muted/60") do
            tr do
              @columns.each do |col|
                th(
                  class: class_names(
                    "px-6 py-3 text-xs font-medium text-muted-foreground",
                    TEXT_ALIGN[col[:align] || :left],
                    col[:width]
                  )
                ) { col[:label] }
              end
            end
          end

          tbody(class: "divide-y divide-border") do
            if @rows.any?
              @rows.each do |row|
                tr(class: row_classes, **row_data_attributes(row)) do
                  @columns.each do |col|
                    td(class: class_names(
                      "px-6 py-4 whitespace-nowrap text-sm text-foreground",
                      TEXT_ALIGN[col[:align] || :left]
                    )) do
                      if @cell_block
                        instance_exec(row, col, &@cell_block)
                      else
                        plain(cell_value(row, col).to_s)
                      end
                    end
                  end
                end
              end
            else
              tr do
                td(
                  colspan: @columns.length,
                  class: "px-6 py-8 text-center text-sm text-muted-foreground"
                ) { plain(@empty_state || t(".no_data")) }
              end
            end
          end
        end
      end
    end

    private

    TEXT_ALIGN = {
      left: "text-left",
      center: "text-center",
      right: "text-right"
    }.freeze

    def row_classes
      class_names("hover:bg-muted/50 transition-colors", @row_url && "cursor-pointer")
    end

    def row_data_attributes(row)
      return {} unless @row_url

      url = @row_url.call(row)
      url ? { data: { href: url } } : {}
    end

    def cell_value(row, col)
      key = col[:key]
      if row.respond_to?(key)
        row.public_send(key)
      elsif row.respond_to?(:[])
        row[key]
      end
    end
  end
end
