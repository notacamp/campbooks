# frozen_string_literal: true

module Files
  # Preview for the Files filter chips — the removable active-filter badges that
  # render inside the `files_results` Turbo Frame alongside the result list.
  class FilterChipsPreview < Lookbook::Preview
    FakeDocumentType           = Struct.new(:id, :name, :color, keyword_init: true)
    FakeDocumentTypeWithSchema = Struct.new(:id, :name, :color, :extraction_schema, keyword_init: true)
    FakeFolder                 = Struct.new(:id, :name, keyword_init: true)

    # No active filters — component renders nothing.
    def empty
      render Campbooks::Files::FilterChips.new(
        filters: Documents::Filters.new,
        q: nil,
        folder: nil,
        document_types: [],
        folders: []
      )
    end

    # Several chips — review status, starred, date range, amount range.
    def with_chips
      filters = Documents::Filters.from_params(
        review_status: "approved",
        starred: "1",
        date_from: "2026-01-01",
        date_to: "2026-06-30",
        amount_min: "100",
        amount_max: "5000"
      )
      render Campbooks::Files::FilterChips.new(
        filters: filters,
        q: nil,
        folder: nil,
        document_types: [],
        folders: []
      )
    end

    # Type chips with ColorDot labels, entity, and source.
    def with_type_and_entity
      filters = Documents::Filters.from_params(
        type: [ "aaaa", "bbbb" ],
        source: "email",
        entity: "EDP Comercial"
      )
      render Campbooks::Files::FilterChips.new(
        filters: filters,
        q: "contract",
        folder: nil,
        document_types: [
          FakeDocumentType.new(id: "aaaa", name: "Invoice",  color: "#3b82f6"),
          FakeDocumentType.new(id: "bbbb", name: "Receipt",  color: "#10b981")
        ],
        folders: []
      )
    end

    # Field filter chips — per-schema-field op chips shown when single type selected.
    def with_field_filters
      invoice_type = FakeDocumentTypeWithSchema.new(
        id: "aaaa", name: "Invoice", color: "#3b82f6",
        extraction_schema: {
          "vendor_name"  => { "type" => "string", "description" => "Vendor Name", "position" => 1 },
          "invoice_date" => { "type" => "date",   "description" => "Invoice Date", "position" => 2 },
          "total_amount" => { "type" => "money",  "description" => "Amount", "position" => 3 }
        }
      )
      filters = Documents::Filters.from_params(
        type: [ "aaaa" ],
        f: {
          "vendor_name"  => { "contains" => "EDP" },
          "invoice_date" => { "from" => "2026-01-01", "to" => "2026-06-30" },
          "total_amount" => { "min" => "100", "max" => "5000" }
        }
      )
      render Campbooks::Files::FilterChips.new(
        filters: filters,
        q: nil,
        folder: nil,
        document_types: [ invoice_type ],
        folders: [],
        single_type: invoice_type
      )
    end

    # All chip types at once — stress test of wrapping at narrow widths.
    def all_chip_types
      filters = Documents::Filters.from_params(
        review_status: "pending",
        ai_status: "completed",
        source: "email",
        starred: "1",
        date_from: "2026-01-01",
        date_to: "2026-12-31",
        amount_min: "50",
        amount_max: "10000",
        entity: "EDP",
        number: "INV-001",
        expense_category: "travel"
      )
      render Campbooks::Files::FilterChips.new(
        filters: filters,
        q: nil,
        folder: nil,
        document_types: [],
        folders: []
      )
    end
  end
end
