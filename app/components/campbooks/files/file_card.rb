# frozen_string_literal: true

module Campbooks
  module Files
    # One file (Document) as a card for the mobile list (below md). Mirrors FileRow's
    # content in a stacked layout; shares the icon/kind/size helpers and the actions
    # menu so the two never drift.
    #
    # When `field_columns:` is passed (a single-type filtered view), the first
    # money or date field's formatted value is appended to the meta line.
    class FileCard < Campbooks::Base
      include FilePresentation

      def initialize(doc:, folders: [], current_folder: nil, field_columns: [])
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
        @field_columns = field_columns || []
      end

      def view_template
        div(id: helpers.dom_id(@doc, :card),
          class: "flex items-center gap-3 rounded-xl border border-gray-200 bg-card p-3 shadow-sm dark:border-white/10") do
          # Inside the `files_results` frame — break out (see FileRow).
          a(href: helpers.document_path(@doc), class: "flex min-w-0 flex-1 items-center gap-3", data: { turbo_frame: "_top" }) do
            span(class: "flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") { raw(safe(type_icon)) }
            span(class: "min-w-0 flex-1") do
              span(class: "block truncate text-sm font-medium text-foreground") { @doc.display_title }
              span(class: "mt-0.5 block truncate text-xs text-muted-foreground") { meta_line }
            end
          end
          div(class: "flex-shrink-0") do
            render(Campbooks::Files::FileActionsMenu.new(doc: @doc, folders: @folders, current_folder: @current_folder))
          end
        end
      end

      private

      def meta_line
        parts = [ kind_label, human_size, l(@doc.created_at.to_date, format: :long) ]

        primary_field = @field_columns.find { |f| f.kind == :money || f.kind == :date }
        if primary_field
          value = primary_field.read(@doc.metadata)
          formatted = format_primary_field(primary_field, value) if value
          parts << formatted if formatted.present?
        end

        parts.compact.join(" · ")
      end

      def format_primary_field(field, value)
        case field.kind
        when :money then helpers.format_currency(value)
        when :date  then helpers.l(value, format: :long)
        end
      end
    end
  end
end
