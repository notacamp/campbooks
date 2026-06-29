# frozen_string_literal: true

module Campbooks
  module Files
    # The Files search bar — a plain GET <form> that navigates the Files page with a
    # `q` query, running the semantic + keyword document search behind
    # Documents::Search. Sits above the filter strip. Any active filter params ride
    # along as hidden fields so a search keeps the current filters applied.
    class SearchBar < Campbooks::Base
      SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'
      CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'

      # @param q [String, nil] the current query
      # @param folder [MailFolder, nil] when set, search stays scoped to that folder
      # @param filter_params [Hash] active filter params to carry through (category, type, …)
      def initialize(q: nil, folder: nil, filter_params: {})
        @q = q.to_s.strip.presence
        @folder = folder
        @filter_params = filter_params || {}
      end

      def view_template
        form(
          method: "get", action: action_path, role: "search",
          class: "mb-4 flex items-center gap-2 rounded-xl border border-gray-200 bg-card px-3 py-2 shadow-sm dark:border-white/10"
        ) do
          icon(SEARCH_ICON, "w-4 h-4 text-gray-400 flex-shrink-0")
          input(
            type: "search", name: "q", value: @q,
            placeholder: t(".placeholder"), autocomplete: "off", enterkeyhint: "search",
            aria_label: t(".placeholder"),
            class: "block w-full min-w-0 flex-1 border-0 bg-transparent p-0 text-sm text-gray-700 placeholder:text-gray-400 focus:outline-none focus:ring-0 dark:text-gray-200"
          )
          hidden_filters
          clear_link if @q
          render(Campbooks::Button.new(variant: :primary, size: :sm, type: :submit)) { t(".submit") }
        end
      end

      private

      def action_path
        @folder ? helpers.files_folder_path(@folder) : helpers.files_path
      end

      # Carry active filters through, so submitting a search keeps them applied. An
      # array value (e.g. multi-select type) becomes repeated `name[]` inputs.
      def hidden_filters
        @filter_params.each do |key, value|
          Array(value).each do |v|
            next if v.to_s.blank?
            input(type: "hidden", name: value.is_a?(Array) ? "#{key}[]" : key.to_s, value: v)
          end
        end
      end

      def clear_link
        a(
          href: action_path, title: t(".clear"), aria_label: t(".clear"),
          class: "flex h-6 w-6 flex-shrink-0 items-center justify-center rounded text-gray-400 hover:bg-gray-100 hover:text-gray-600 dark:hover:bg-white/10"
        ) { icon(CLEAR_ICON, "w-3.5 h-3.5") }
      end

      def icon(path, classes)
        svg(class: classes, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(path)) }
      end
    end
  end
end
