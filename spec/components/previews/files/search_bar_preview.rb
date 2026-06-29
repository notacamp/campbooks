# frozen_string_literal: true

module Files
  # Preview for the Files search bar — the GET form that runs the semantic +
  # keyword document search (Documents::Search) on the Files page.
  class SearchBarPreview < Lookbook::Preview
    # @param q text
    def default(q: "")
      render Campbooks::Files::SearchBar.new(q: q, folder: nil, filter_params: {})
    end

    # With a query present — the clear (×) button appears.
    def with_query
      render Campbooks::Files::SearchBar.new(q: "invoice EDP 2026", folder: nil, filter_params: {})
    end

    # Active filters carried through as hidden fields, so a search keeps them applied.
    def with_filters
      render Campbooks::Files::SearchBar.new(
        q: "contract", folder: nil,
        filter_params: { "category" => "legal", "type" => [ "1", "2" ] }
      )
    end
  end
end
