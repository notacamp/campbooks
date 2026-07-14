# frozen_string_literal: true

module Campbooks
  module Files
    # One file (Document) as a grid-view tile: a thumbnail of the file's first
    # page (see Document#thumbnail) over a compact caption. Non-representable
    # files (office types, archives, or a host without poppler) fall back to the
    # shared type icon, so the tile degrades to a bigger FileCard. Shares the
    # icon/kind/size helpers and the actions menu with FileRow/FileCard.
    #
    # The whole tile is one link to the document page; the kebab floats over the
    # caption's reserved right edge and reveals on hover/focus (always visible on
    # touch, and pinned while its menu is open).
    class FileTile < Campbooks::Base
      include FilePresentation
      include Phlex::Rails::Helpers::URLFor

      def initialize(doc:, folders: [], current_folder: nil, field_columns: [])
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
        @field_columns = field_columns || []
      end

      def view_template
        div(id: helpers.dom_id(@doc, :tile),
          class: "group relative flex flex-col rounded-xl border border-gray-200 bg-card shadow-sm transition-shadow hover:border-gray-300 hover:shadow-md dark:border-white/10 dark:hover:border-white/25") do
          # Inside the `files_results` frame — break out (see FileRow).
          a(href: helpers.document_path(@doc), class: "block min-w-0", data: { turbo_frame: "_top" }) do
            thumbnail_area
            div(class: "flex items-center gap-1.5 py-2 pl-2.5 pr-9") do
              span(class: "flex-shrink-0 text-muted-foreground") { raw(safe(type_icon)) }
              span(class: "min-w-0 flex-1") do
                span(class: "block truncate text-sm font-medium text-foreground") { @doc.display_title }
                span(class: "mt-0.5 block truncate text-xs text-muted-foreground") { meta_line }
              end
            end
          end
          div(class: "absolute bottom-1.5 right-1 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 pointer-coarse:opacity-100 has-[details[open]]:opacity-100") do
            render(Campbooks::Files::FileActionsMenu.new(doc: @doc, folders: @folders, current_folder: @current_folder))
          end
        end
      end

      private

      # The 4:3 preview well. A document with a generated thumbnail (see
      # Documents::ThumbnailGenerator) renders it top-anchored, so the crop
      # keeps the recognizable part of a page (letterhead, title); the browser
      # only fetches images near the viewport.
      def thumbnail_area
        div(class: "relative aspect-[4/3] overflow-hidden rounded-t-[calc(0.75rem-1px)] border-b border-gray-200 bg-muted dark:border-white/10") do
          if @doc.thumbnail.attached?
            img(src: url_for(@doc.thumbnail), alt: "", loading: "lazy", decoding: "async",
              class: "absolute inset-0 h-full w-full object-cover object-top")
          else
            div(class: "absolute inset-0 flex flex-col items-center justify-center gap-2 text-muted-foreground") do
              span(class: "[&_svg]:h-8 [&_svg]:w-8") { raw(safe(type_icon)) }
              span(class: "text-[10px] font-semibold uppercase tracking-widest") { kind_label }
            end
          end
        end
      end

      def meta_line
        [ kind_label, human_size, primary_field_value(@field_columns) ].compact.join(" · ")
      end
    end
  end
end
