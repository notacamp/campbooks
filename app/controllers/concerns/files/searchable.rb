# frozen_string_literal: true

module Files
  # Builds one Documents::Search per request and exposes the derived filter/sort/schema
  # state the Files and Paper lists both render from. Extracted from FilesController so
  # PaperController reuses the exact search-building logic rather than copying it.
  #
  # Paper passes `search_params: params.except(:type)` because Paper's `type` param is a
  # bucket key (invoices/receipts/…), NOT the document_type UUIDs Documents::Filters
  # expects — Paper applies its bucket filter itself (see PaperController).
  module Searchable
    extend ActiveSupport::Concern

    private

    def build_search(search_params: params)
      @search       = Documents::Search.new(
        user: Current.user, workspace: Current.workspace, params: search_params, folder: @folder
      )
      @filters      = @search.filters
      @search_query = @search.search_text # parsed free text (modifiers stripped)
      @sorter       = @search.sorter
      @single_type  = @filters.single_type(Current.workspace)
      # Cap at 3 so the fixed-layout table keeps Name readable (see index name_width).
      @field_columns = @single_type ? DocumentTypes::Schema.for(@single_type).fields.first(3) : []
    end
  end
end
