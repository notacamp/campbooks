# frozen_string_literal: true

module Campbooks
  module Organizations
    # Live search for the Organizations directory. A debounced GET <form> that
    # navigates the `organizations_results` Turbo Frame with a `q` query, filtered
    # by Organization#search (name / domain). The `list-search` controller submits
    # on a short debounce (and immediately on Enter); the frame swap keeps the page
    # header and the input's focus in place while the list filters.
    class SearchBar < Campbooks::Base
      SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'
      CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'

      # @param q [String, nil] the current query
      def initialize(q: nil)
        @q = q.to_s.strip.presence
      end

      def view_template
        form(
          method: "get", action: helpers.organizations_path, role: "search",
          data: { controller: "list-search", turbo_frame: "organizations_results" },
          class: "mb-4 flex items-center gap-2 rounded-xl border border-border bg-card px-3 py-2 shadow-sm"
        ) do
          icon(SEARCH_ICON, "w-4 h-4 text-muted-foreground flex-shrink-0")
          input(
            type: "search", name: "q", value: @q,
            placeholder: t(".placeholder"), autocomplete: "off", enterkeyhint: "search",
            aria_label: t(".placeholder"),
            data: { list_search_target: "input", action: "input->list-search#submit keydown.enter->list-search#submitNow" },
            # Hide the native WebKit clear (×) — our own Clear button replaces it and
            # also works during live search (the form sits outside the frame).
            class: "block w-full min-w-0 flex-1 border-0 bg-transparent p-0 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0 [&::-webkit-search-cancel-button]:appearance-none"
          )
          clear_button
        end
      end

      private

      # Always rendered; the list-search controller shows it only when the field
      # has a value (starts hidden unless a query was pre-filled server-side).
      def clear_button
        button(
          type: "button", title: t(".clear"), aria_label: t(".clear"),
          data: { list_search_target: "clear", action: "list-search#clear" },
          class: "#{'hidden ' unless @q}flex h-6 w-6 flex-shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
        ) { icon(CLEAR_ICON, "w-3.5 h-3.5") }
      end

      def icon(path, classes)
        svg(class: classes, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(path)) }
      end
    end
  end
end
