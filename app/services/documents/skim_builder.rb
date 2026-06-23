# frozen_string_literal: true

module Documents
  # Groups the document review queue into Skim's "rings" for the stories viewer.
  #
  # A ring is a CATEGORY (DocumentType::CATEGORIES: accounting, legal, insurance,
  # vehicles, identification, correspondence, other). The tray shows these
  # categories; opening one walks its documents one card at a time. Unlike email
  # Skim there is NO clustering — every document is verified individually, so one
  # document = one card.
  #
  # Documents arrive already ordered (most-uncertain-first) from SkimScope; the
  # builder preserves that order within each ring and assigns per-ring
  # position/total for the viewer's progress bar. Dependency-light + deterministic
  # so it unit-tests in isolation against plain structs.
  class SkimBuilder
    # Category rings, in the canonical taxonomy order. An unclassified or
    # unknown-category document falls into the trailing "other" ring.
    CATEGORY_ORDER = DocumentType::CATEGORIES

    def initialize(documents)
      @documents = documents.to_a
    end

    # Category rings in CATEGORY_ORDER (empty categories omitted), each holding its
    # documents as one-per-card clusters with per-ring position/total.
    def rings
      grouped = @documents.group_by { |doc| category_of(doc) }
      CATEGORY_ORDER.filter_map do |category|
        docs = grouped[category]
        build_ring(category, docs) if docs&.any?
      end
    end

    # Flat, globally-ordered list of cards (the "Review all" walk).
    def clusters
      rings.flat_map { |ring| ring[:clusters] }
    end

    private

    def category_of(doc)
      cat = doc.classification&.category.to_s
      CATEGORY_ORDER.include?(cat) ? cat : "other"
    end

    def build_ring(category, docs)
      total = docs.size
      {
        category: category,
        label: category.humanize,
        count: total,
        clusters: docs.each_with_index.map { |doc, i| build_card(doc, category, i + 1, total) }
      }
    end

    def build_card(doc, category, position, total)
      {
        document_id: doc.id,
        category: category,
        display_title: doc.display_title,
        entity_display_name: doc.entity_display_name,
        reference_display: doc.reference_display,
        document_date: doc.document_date,
        amount_display: doc.amount&.format,
        ai_confidence_score: doc.ai_confidence_score,
        type_label: type_label(doc),
        type_color: doc.classification&.color,
        type_id: doc.document_type_id,
        is_image: doc.image?,
        is_pdf: doc.pdf?,
        filename: doc.original_file.filename.to_s,
        # The AI-extracted fields the reviewer signs off on (display + inline edit).
        extracted_fields: extracted_fields(doc),
        # Raw value for the inline name editor (the display fields above are computed).
        title_value: doc.metadata&.dig("title"),
        position: position,
        total: total
      }
    end

    def type_label(doc)
      (doc.classification&.name || doc.document_type).to_s.humanize
    end

    # The type-specific data the AI pulled out — the SAME field set the document
    # detail page shows (per-type columns, or a custom type's extraction_schema),
    # surfaced on the card so the reviewer can verify (and correct) it before
    # approving. Centralised in Documents::ExtractedFieldSet so Skim and the detail
    # page never drift. Blank values are kept (the card shows the full field set and
    # pre-fills the inline editor).
    def extracted_fields(doc)
      Documents::ExtractedFieldSet.new(doc).fields
    end
  end
end
