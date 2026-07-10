# frozen_string_literal: true

module Campbooks
  module Files
    # The Files search bar — a GET <form> that navigates the `files_results` Turbo
    # Frame in place (sidebar + header stay put). Holds the text query, a typeahead
    # for modifier tokens (powered by the files-search Stimulus controller, a
    # subclass of email-search), a Filters button that reveals the FilterPanel, and
    # a clear link.
    #
    # Active-filter chips render *inside* the results frame (FilterChips component),
    # not here, so they refresh together with the results.
    class SearchBar < Campbooks::Base
      SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'.freeze
      CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'.freeze
      FILTER_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4.5h18M6.75 9.75h10.5M10.5 15h3"/>'.freeze

      # @param q [String, nil] current free-text query (modifiers stripped)
      # @param folder [MailFolder, nil] when set, search stays scoped to that folder
      # @param filters [Documents::Filters] active structured filters
      # @param document_types [Array] workspace document types for type checkboxes
      # @param folders [Array] accessible folders for typeahead + panel select
      # @param categories [Array<String>] DocumentType::CATEGORIES
      def initialize(q: nil, folder: nil, filters: nil, document_types: [], folders: [], categories: [], **attrs)
        @q              = q.to_s.strip.presence
        @folder         = folder
        @filters        = filters || Documents::Filters.new
        @document_types = document_types
        @folders        = folders
        @categories     = categories
        @attrs          = attrs
      end

      def view_template
        form(
          method: "get",
          action: action_path,
          class: class_names("border-b border-gray-100 relative", @attrs.delete(:class)),
          data: {
            controller: "files-search",
            turbo_frame: "files_results",
            turbo_action: "replace",
            action: "change->files-search#submitNow",
            files_search_suggestions_value: catalog.to_json,
            files_search_frame_id_value: "files_results",
            files_search_heading_text_value: t(".suggest.heading")
          },
          **@attrs
        ) do
          input_row
          suggestions_panel
          panel
        end
      end

      private

      def input_row
        div(class: "flex items-center gap-1.5 px-2.5 py-1.5 relative") do
          span(class: "flex-shrink-0", data: { files_search_target: "searchIcon" }) do
            icon(SEARCH_ICON, "w-3.5 h-3.5 text-gray-400")
          end
          span(
            class: "flex-shrink-0 hidden",
            aria_live: "polite",
            data: { files_search_target: "spinner" }
          ) do
            render(Campbooks::Spinner.new(size: :sm, class: "w-[0.875rem] h-[0.875rem]"))
            span(class: "sr-only") { t(".searching") }
          end
          div(class: "flex-1 min-w-0") do
            input(
              type: "text",
              name: "q",
              value: @q,
              placeholder: t(".placeholder"),
              autocomplete: "off",
              autocapitalize: "off",
              spellcheck: "false",
              role: "combobox",
              aria_expanded: "false",
              aria_controls: "files-search-suggestions",
              aria_autocomplete: "list",
              class: "block w-full bg-transparent border-0 p-0 text-xs text-gray-700 dark:text-gray-200 placeholder:text-gray-400 focus:ring-0 focus:outline-none",
              data: {
                files_search_target: "query",
                action: "input->files-search#scheduleSubmit keydown->files-search#handleKeydown focus->files-search#openSuggestions blur->files-search#closeSuggestionsSoon"
              }
            )
          end
          clear_link if @q.present?
          filters_button
        end
      end

      def suggestions_panel
        div(
          id: "files-search-suggestions",
          role: "listbox",
          class: "absolute left-0 right-0 top-full z-30 mt-1 mx-1.5 hidden rounded-xl border border-gray-100 bg-card shadow-lg overflow-hidden",
          data: { files_search_target: "suggestions" }
        ) do
          div(
            class: "max-h-64 overflow-y-auto overscroll-contain py-1",
            data: { files_search_target: "suggestionsList" }
          )
        end
      end

      def clear_link
        a(
          href: action_path,
          title: t(".clear"),
          aria_label: t(".clear"),
          class: "flex items-center justify-center w-5 h-5 rounded text-gray-400 hover:text-gray-600 hover:bg-gray-100 flex-shrink-0",
          data: { turbo_frame: "_top" }
        ) { icon(CLEAR_ICON, "w-3 h-3") }
      end

      def filters_button
        render(Campbooks::Button.new(
          variant: :ghost, size: :xs, type: :button,
          class: "flex-shrink-0 gap-1 text-gray-500 hover:text-gray-700",
          data: { action: "click->files-search#toggleFilters" }
        )) do
          icon(FILTER_ICON, "w-3.5 h-3.5")
          span(class: "hidden sm:inline") { t(".filters") }
          render(Campbooks::Badge.new(variant: :accent, size: :sm)) { @filters.active_count.to_s } if @filters.active_count.positive?
        end
      end

      def panel
        div(class: "border-t border-gray-100 px-2.5 py-2.5 hidden", data: { files_search_target: "filterPanel" }) do
          render(Campbooks::Files::FilterPanel.new(
            folder: @folder,
            filters: @filters,
            q: @q,
            document_types: @document_types,
            folders: @folders,
            categories: @categories
          ))
        end
      end

      # Modifier catalog passed to Stimulus as a JSON value. Each entry describes one
      # modifier token with enough metadata for the JS to render typeahead rows.
      def catalog
        [
          { token: "type:",     type: "enum",  description: t(".suggest.type"),
            values: @document_types.map { |dt| { value: dt.name.downcase, label: dt.name.humanize } } },
          { token: "category:", type: "enum",  description: t(".suggest.category"),
            values: @categories.map { |c| { value: c, label: helpers.human_enum(DocumentType, :category, c) } } },
          { token: "vendor:",   type: "text",  description: t(".suggest.vendor") },
          { token: "number:",   type: "text",  description: t(".suggest.number") },
          { token: "amount>",   type: "text",  description: t(".suggest.amount_gt"), hint: "100" },
          { token: "amount<",   type: "text",  description: t(".suggest.amount_lt"), hint: "100" },
          { token: "after:",    type: "date",  description: t(".suggest.after"),    hint: "YYYY-MM-DD" },
          { token: "before:",   type: "date",  description: t(".suggest.before"),   hint: "YYYY-MM-DD" },
          { token: "is:",       type: "enum",  description: t(".suggest.is"),
            values: [
              { value: "starred",    label: t(".suggest.values.starred") },
              { value: "pending",    label: t(".suggest.values.pending") },
              { value: "approved",   label: t(".suggest.values.approved") },
              { value: "rejected",   label: t(".suggest.values.rejected") },
              { value: "failed",     label: t(".suggest.values.failed") },
              { value: "processing", label: t(".suggest.values.processing") }
            ] },
          { token: "source:",   type: "enum",  description: t(".suggest.source"),
            values: [
              { value: "email",  label: t(".suggest.values.email") },
              { value: "upload", label: t(".suggest.values.upload") },
              { value: "notion", label: t(".suggest.values.notion") },
              { value: "sent",   label: t(".suggest.values.sent") }
            ] },
          { token: "expense:",  type: "enum",  description: t(".suggest.expense"),
            values: Document.expense_categories.keys.map { |k|
              { value: k, label: helpers.human_enum(Document, :expense_category, k) }
            } },
          { token: "in:",       type: "enum",  description: t(".suggest.in"),
            values: @folders.map { |f| { value: f.name, label: f.name } } }
        ]
      end

      def action_path
        @folder ? helpers.files_folder_path(@folder) : helpers.files_path
      end

      def icon(path, classes)
        svg(class: classes, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(path)) }
      end
    end
  end
end
