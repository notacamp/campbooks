# frozen_string_literal: true

module Campbooks
  module Files
    # One file (Document) as a desktop table row (md+). The name links to the
    # document detail page (preview + metadata + folder management); the kebab cell
    # renders the shared Campbooks::Files::FileActionsMenu.
    #
    # When `field_columns:` is passed (a single-type filtered view), the Kind and
    # Size columns are replaced by per-field columns with typed value rendering.
    class FileRow < Campbooks::Base
      include FilePresentation

      def initialize(doc:, folders: [], current_folder: nil, field_columns: [])
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
        @field_columns = field_columns || []
      end

      def view_template
        tr(id: helpers.dom_id(@doc), class: "group hover:bg-accent-50 dark:hover:bg-white/5") do
          td(class: "px-6 py-3 max-w-0") do
            # Rows render inside the `files_results` frame; break out or Turbo
            # would look for that frame on the document page ("Content missing").
            a(href: helpers.document_path(@doc), class: "flex min-w-0 items-center gap-3", data: { turbo_frame: "_top" }) do
              span(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") { raw(safe(type_icon)) }
              span(class: "truncate text-sm font-medium text-foreground") { @doc.display_title }
            end
          end

          if @field_columns.any?
            @field_columns.each do |field|
              td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") do
                format_field_cell(field)
              end
            end
          else
            td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { kind_label }
            td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { human_size }
          end

          td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { l(@doc.created_at.to_date, format: :long) }
          td(class: "px-3 py-3 text-right whitespace-nowrap") do
            render(Campbooks::Files::FileActionsMenu.new(doc: @doc, folders: @folders, current_folder: @current_folder))
          end
        end
      end

      private

      # Format a schema field value from doc.metadata for display in a table cell.
      # Returns an em-dash for nil/blank values.
      def format_field_cell(field)
        value = field.read(@doc.metadata)
        return "—" if value.nil?

        case field.kind
        when :money
          helpers.format_currency(value)
        when :date
          helpers.l(value, format: :long)
        when :boolean
          value ? "✓" : "—"
        when :number, :integer
          value.to_s
        else
          value.to_s.truncate(30)
        end
      end
    end
  end
end
