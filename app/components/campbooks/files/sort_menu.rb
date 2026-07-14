# frozen_string_literal: true

module Campbooks
  module Files
    # Compact sort dropdown for the Files GRID view, where the table's sortable
    # headers aren't rendered. Offers the same keys as the headers — name, added,
    # plus the typed field columns of a single-type filtered view — with the same
    # semantics: picking the active key flips the direction, picking a new key
    # starts ascending. Links carry the full filter/query state and load as a
    # full page (turbo_frame: _top), exactly like Campbooks::Files::SortableHeader.
    #
    # Only visible in grid mode: the wrapper carries data-files-pane="sort" and
    # application.css reveals it under [data-files-layout="grid"].
    class SortMenu < Campbooks::Base
      def initialize(sorter:, filters:, q: nil, folder: nil, field_columns: [])
        @sorter = sorter
        @filters = filters
        @q = q
        @folder = folder
        @field_columns = field_columns || []
      end

      def view_template
        details(class: "relative text-left", data: { controller: "dropdown-close", files_pane: "sort" }) do
          summary(class: "inline-flex cursor-pointer list-none items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-sm font-medium text-foreground hover:bg-muted [&::-webkit-details-marker]:hidden",
            aria: { label: t(".aria_label") }) do
            span(class: "text-muted-foreground") { t(".sort") }
            span { active_label }
            raw(safe(direction_icon))
          end
          div(class: "absolute right-0 z-20 mt-1 w-44 rounded-lg border border-border bg-card p-1 text-left shadow-lg") do
            options.each { |key, label| sort_link(key, label) }
          end
        end
      end

      private

      def options
        base = [ [ "name", t(".name") ], [ "added", t(".added") ] ]
        base + @field_columns.map { |field| [ field.key, field.label ] }
      end

      def active_label
        options.to_h.fetch(active_key, t(".added"))
      end

      # The list default (no explicit sort) is newest-first "added".
      def active_key
        @sorter.active? ? @sorter.key : "added"
      end

      def sort_link(key, label)
        active = key == active_key
        new_dir = active ? (@sorter.active? && @sorter.dir == "asc" ? "desc" : "asc") : "asc"
        params = helpers.files_filter_params(@filters, q: @q).merge(sort: key, dir: new_dir)
        href = @folder ? helpers.files_folder_path(@folder, params) : helpers.files_path(params)

        a(href: href, data: { turbo_frame: "_top" },
          class: class_names("flex w-full cursor-pointer items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-muted",
            active ? "font-semibold text-foreground" : "text-foreground")) do
          span { label }
          raw(safe(direction_icon)) if active
        end
      end

      # Chevron reflecting the active direction (down for the descending default).
      def direction_icon
        path = @sorter.active? && @sorter.dir == "asc" ? "M5 15l7-7 7 7" : "M19 9l-7 7-7-7"
        %(<svg class="h-3 w-3 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="#{path}"/></svg>)
      end
    end
  end
end
