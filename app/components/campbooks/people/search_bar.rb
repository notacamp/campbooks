# frozen_string_literal: true

module Campbooks
  module People
    # Live search for the People list. A debounced GET <form> that navigates the
    # "people_results" Turbo Frame with a `q` query (persons by name/email/org,
    # organizations by name/domain). Sits OUTSIDE the frame so the input keeps its
    # focus while the list filters in place. Mirrors Campbooks::Organizations::SearchBar.
    class SearchBar < Campbooks::Base
      SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'
      CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'

      def initialize(q: nil)
        @q = q.to_s.strip.presence
      end

      def view_template
        form(
          method: "get", action: helpers.people_path, role: "search",
          data: { controller: "list-search", turbo_frame: "people_results" },
          class: "flex items-center gap-2 rounded-xl border border-input bg-card px-3 py-2"
        ) do
          icon(SEARCH_ICON, "w-4 h-4 text-muted-foreground flex-shrink-0")
          input(
            type: "search", name: "q", value: @q,
            placeholder: t(".placeholder"), autocomplete: "off", enterkeyhint: "search",
            aria_label: t(".placeholder"),
            data: { list_search_target: "input", action: "input->list-search#submit keydown.enter->list-search#submitNow" },
            class: "block w-full min-w-0 flex-1 border-0 bg-transparent p-0 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-0 [&::-webkit-search-cancel-button]:appearance-none"
          )
          clear_button
        end
      end

      private

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
