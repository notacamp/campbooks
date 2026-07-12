# frozen_string_literal: true

module Campbooks
  module Files
    # A <th> element with a clickable sort link for the files table.
    #
    # Clicking cycles asc→desc when the column is already active; clicking a new
    # column starts ascending. The link carries all current filter + query params
    # so the sort preference survives filter changes and pagination.
    #
    # Props:
    #   label:       [String]                  column heading text
    #   sort_key:    [String]                  value for the `sort=` param
    #   sorter:      [Documents::Sorter]        current sort state
    #   filters:     [Documents::Filters]       active panel filters (for URL round-trip)
    #   q:           [String, nil]              raw query string
    #   folder:      [MailFolder, nil]          when set, links use the folder-scoped path
    #   width_class: [String, nil]              optional Tailwind width class on the <th>
    class SortableHeader < Campbooks::Base
      CHEVRON_UP   = "M5 15l7-7 7 7"
      CHEVRON_DOWN = "M19 9l-7 7-7-7"

      def initialize(label:, sort_key:, sorter:, filters:, q: nil, folder: nil, width_class: nil)
        @label       = label
        @sort_key    = sort_key
        @sorter      = sorter
        @filters     = filters
        @q           = q
        @folder      = folder
        @width_class = width_class
      end

      def view_template
        th(
          class: class_names(
            "px-6 py-3 text-left text-xs font-medium uppercase whitespace-nowrap",
            @width_class,
            active? ? "text-foreground" : "text-gray-500"
          ),
          aria_sort: aria_sort_value
        ) do
          a(
            href: sort_url,
            class: class_names(
              "inline-flex items-center gap-0.5 group transition-colors",
              active? ? "text-foreground font-semibold" : "text-gray-500 hover:text-foreground"
            ),
            data: { turbo_frame: "_top" },
            aria_label: aria_label
          ) do
            span { @label }
            svg(
              class: class_names(
                "h-3 w-3 flex-shrink-0 transition-opacity",
                active? ? "opacity-100" : "opacity-40 group-hover:opacity-70"
              ),
              fill: "none",
              stroke: "currentColor",
              viewBox: "0 0 24 24",
              aria_hidden: "true"
            ) do |s|
              s.path(
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: chevron_path
              )
            end
          end
        end
      end

      private

      def active?
        @sorter.active? && @sorter.key == @sort_key
      end

      def aria_sort_value
        return "none" unless active?

        @sorter.dir == "asc" ? "ascending" : "descending"
      end

      def aria_label
        if active? && @sorter.dir == "asc"
          t(".sort_desc", label: @label)
        elsif active? && @sorter.dir == "desc"
          t(".sort_asc", label: @label)
        else
          t(".sort_by", label: @label)
        end
      end

      def chevron_path
        return CHEVRON_DOWN unless active?

        @sorter.dir == "asc" ? CHEVRON_UP : CHEVRON_DOWN
      end

      def new_dir
        active? ? (@sorter.dir == "asc" ? "desc" : "asc") : "asc"
      end

      def sort_url
        params = helpers.files_filter_params(@filters, q: @q).merge(sort: @sort_key, dir: new_dir)
        @folder ? helpers.files_folder_path(@folder, params) : helpers.files_path(params)
      end
    end
  end
end
