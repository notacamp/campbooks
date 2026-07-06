# frozen_string_literal: true

module Organizations
  # Preview for the Organizations directory search bar — the debounced GET form
  # that filters the directory by name / domain (Organization#search).
  class SearchBarPreview < Lookbook::Preview
    # @param q text
    def default(q: "")
      render Campbooks::Organizations::SearchBar.new(q: q)
    end

    # With a query present — the clear (×) button appears.
    def with_query
      render Campbooks::Organizations::SearchBar.new(q: "acme")
    end
  end
end
