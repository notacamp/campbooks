# frozen_string_literal: true

module Campbooks
  module Files
    # One file (Document) as a card for the mobile list (below md). Mirrors FileRow's
    # content in a stacked layout; shares the icon/kind/size helpers and the actions
    # menu so the two never drift.
    class FileCard < Campbooks::Base
      include FilePresentation

      def initialize(doc:, folders: [], current_folder: nil)
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
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
        [ kind_label, human_size, l(@doc.created_at.to_date, format: :long) ].compact.join(" · ")
      end
    end
  end
end
