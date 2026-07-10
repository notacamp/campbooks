# frozen_string_literal: true

module Files
  # Preview for the Files filter panel — the structured controls that live inside
  # the SearchBar's <form> and auto-submit on change.
  class FilterPanelPreview < Lookbook::Preview
    # Empty state — no filters active, all selects show "any" defaults.
    def default
      render Campbooks::Files::FilterPanel.new(
        folder: nil,
        filters: Documents::Filters.new,
        document_types: sample_document_types,
        folders: sample_folders,
        categories: DocumentType::CATEGORIES
      )
    end

    # Active filters — several dimensions selected; shows the "Clear filters" footer.
    def with_active_filters
      filters = Documents::Filters.from_params(
        review_status: "approved",
        starred: "1",
        date_from: "2026-01-01",
        date_to: "2026-06-30",
        amount_min: "100",
        amount_max: "5000",
        source: "email"
      )
      render Campbooks::Files::FilterPanel.new(
        folder: nil,
        filters: filters,
        document_types: sample_document_types,
        folders: sample_folders,
        categories: DocumentType::CATEGORIES
      )
    end

    # Folder-scoped view — folder select hidden (browsing a specific folder).
    def folder_scoped
      fake_folder = Struct.new(:id, :name, keyword_init: true).new(
        id: "00000000-0000-0000-0000-000000000001", name: "Invoices 2026"
      )
      render Campbooks::Files::FilterPanel.new(
        folder: fake_folder,
        filters: Documents::Filters.new,
        document_types: sample_document_types,
        folders: sample_folders,
        categories: DocumentType::CATEGORIES
      )
    end

    private

    FakeDocumentType = Struct.new(:id, :name, :color, keyword_init: true)

    def sample_document_types
      [
        FakeDocumentType.new(id: "aaaa", name: "Invoice",  color: "#3b82f6"),
        FakeDocumentType.new(id: "bbbb", name: "Receipt",  color: "#10b981"),
        FakeDocumentType.new(id: "cccc", name: "Contract", color: "#f59e0b")
      ]
    end

    FakeFolder = Struct.new(:id, :name, keyword_init: true)

    def sample_folders
      [
        FakeFolder.new(id: "f1", name: "Invoices"),
        FakeFolder.new(id: "f2", name: "Receipts"),
        FakeFolder.new(id: "f3", name: "Legal")
      ]
    end
  end
end
