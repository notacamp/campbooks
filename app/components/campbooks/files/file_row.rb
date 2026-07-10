# frozen_string_literal: true

module Campbooks
  module Files
    # One file (Document) as a desktop table row (md+). The name links to the
    # document detail page (preview + metadata + folder management); the kebab cell
    # renders the shared Campbooks::Files::FileActionsMenu.
    class FileRow < Campbooks::Base
      include FilePresentation

      def initialize(doc:, folders: [], current_folder: nil)
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
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
          td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { kind_label }
          td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { human_size }
          td(class: "px-6 py-3 text-sm text-muted-foreground whitespace-nowrap") { l(@doc.created_at.to_date, format: :long) }
          td(class: "px-3 py-3 text-right whitespace-nowrap") do
            render(Campbooks::Files::FileActionsMenu.new(doc: @doc, folders: @folders, current_folder: @current_folder))
          end
        end
      end
    end
  end
end
