# frozen_string_literal: true

class DocSkimCardComponentPreview < ViewComponent::Preview
  # A clean inline placeholder so the image-preview variant doesn't 404 in isolation.
  PLACEHOLDER_IMG = "data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='320'%20height='200'%3E%3Crect%20width='320'%20height='200'%20rx='8'%20fill='%23e5e7eb'/%3E%3Ctext%20x='160'%20y='106'%20font-family='sans-serif'%20font-size='15'%20fill='%239ca3af'%20text-anchor='middle'%3EReceipt%20scan%3C/text%3E%3C/svg%3E"

  # The reclassify picker's options (stands in for workspace DocumentType records).
  Type = Struct.new(:id, :name, :category)
  TYPES = [
    Type.new(1, "expense_invoice", "accounting"),
    Type.new(2, "revenue_invoice", "accounting"),
    Type.new(3, "bank_statement", "accounting"),
    Type.new(4, "contract", "legal"),
    Type.new(5, "insurance_policy", "insurance"),
    Type.new(6, "vehicle_document", "vehicles"),
    Type.new(7, "identification", "identification"),
    Type.new(8, "correspondence", "correspondence"),
    Type.new(9, "other", "other")
  ].freeze

  # The same set with NO category — which is what the setup wizard, the AI analyzer,
  # and onboarding actually create. Grouping strictly by the known categories used to
  # drop these entirely (empty picker); now they list under "Unclassified".
  TYPES_UNCATEGORISED = TYPES.map { |t| Type.new(t.id, t.name, nil) }.freeze

  def self.card(id, **over)
    {
      document_id: id, category: "accounting", display_title: "Invoice #{id}",
      entity_display_name: "Acme Supplies Lda", reference_display: "INV-#{id}",
      document_date: Date.new(2026, 6, 10), amount_display: "€1,240.00",
      ai_confidence_score: 0.52, type_label: "Expense invoice", type_color: "#3b82f6",
      type_id: 1, is_image: false, is_pdf: true, filename: "invoice_#{id}.pdf",
      extracted_fields: [
        { key: "vendor_name", label: "Vendor", value: "Acme Supplies Lda" },
        { key: "invoice_number", label: "Invoice number", value: "INV-#{id}" },
        { key: "document_date", label: "Date", value: "2026-06-10" },
        { key: "amount", label: "Amount", value: "1240.00" },
        { key: "tax_amount", label: "Tax", value: "240.00" },
        { key: "vendor_nif", label: "Tax ID", value: "PT500100200" }
      ],
      position: 1, total: 1
    }.merge(over)
  end

  RINGS = [
    { category: "accounting", label: "Accounting", count: 2, clusters: [
      card(101, position: 1, total: 2, ai_confidence_score: 0.38),
      card(102, position: 2, total: 2, type_label: "Bank statement", type_id: 3, type_color: "#8b5cf6",
           display_title: "BPI statement — May", entity_display_name: "Banco BPI", reference_display: "01/05 – 31/05",
           amount_display: nil, ai_confidence_score: 0.61,
           extracted_fields: [
             { key: "bank_name", label: "Bank", value: "Banco BPI" },
             { key: "account_number", label: "Account", value: "PT50 0010 0000 1234" },
             { key: "period_start", label: "Period start", value: "2026-05-01" },
             { key: "period_end", label: "Period end", value: "2026-05-31" },
             { key: "closing_balance", label: "Closing balance", value: "3210.55" }
           ])
    ] },
    { category: "insurance", label: "Insurance", count: 1, clusters: [
      card(201, category: "insurance", position: 1, total: 1, type_label: "Insurance policy", type_color: "#22c55e",
           type_id: 5, display_title: "Allianz policy AX-200", entity_display_name: "Allianz", reference_display: "AX-200",
           amount_display: "€480.00", ai_confidence_score: 0.55)
    ] },
    { category: "other", label: "Other", count: 1, clusters: [
      card(301, category: "other", position: 1, total: 1, type_label: nil, type_id: nil, type_color: nil,
           display_title: "Scan_0042.pdf", entity_display_name: nil, reference_display: nil, amount_display: nil,
           ai_confidence_score: 0.16, extracted_fields: [])
    ] }
  ].freeze

  # @label Review flow — Stories viewer (full-screen; A approve / → skip / C reclassify)
  def story
    render Campbooks::DocSkimStack.new(rings: RINGS, standalone: true, document_types: TYPES)
  end

  # @label Review flow — uncategorised types (press C: picker lists them under "Unclassified")
  def story_uncategorised_types
    render Campbooks::DocSkimStack.new(rings: RINGS, standalone: true, document_types: TYPES_UNCATEGORISED)
  end

  # @label Review flow — no document types yet (press C: picker shows a placeholder, not a blank box)
  def story_no_types
    render Campbooks::DocSkimStack.new(rings: RINGS, standalone: true, document_types: [])
  end

  # @label Card — PDF (inline preview + Expand/Download controls)
  # The PDF iframe is lazy-loaded by the viewer's controller, so in this isolated
  # card it stays on the placeholder; the live inline render is in the story flow.
  def pdf_card
    render Campbooks::DocSkimCard.new(**self.class.card(1, ai_confidence_score: 0.34), document_types: TYPES, class: "max-w-md")
  end

  # @label Card — image preview
  def image_card
    render Campbooks::DocSkimCard.new(
      **self.class.card(2, is_image: true, is_pdf: false, file_url: PLACEHOLDER_IMG,
                        type_label: "Receipt", type_id: nil, type_color: "#f59e0b",
                        display_title: "Lunch receipt", entity_display_name: "Café Central",
                        reference_display: "REC-88", amount_display: "€18.40", ai_confidence_score: 0.49),
      document_types: TYPES, class: "max-w-md"
    )
  end

  # @label Card — unclassified (no type, very low confidence)
  def unclassified
    render Campbooks::DocSkimCard.new(
      **self.class.card(3, category: "other", type_label: nil, type_id: nil, type_color: nil,
                        display_title: "Scan_0042.pdf", entity_display_name: nil, reference_display: nil,
                        amount_display: nil, ai_confidence_score: 0.16, filename: "scan_0042.pdf",
                        extracted_fields: []),
      document_types: TYPES, class: "max-w-md"
    )
  end

  # @label Card — non-previewable file (icon tile + Download fallback)
  def other_file
    render Campbooks::DocSkimCard.new(
      **self.class.card(4, is_image: false, is_pdf: false, type_label: "Contract", type_id: 4, type_color: "#8b5cf6",
                        display_title: "Lease agreement.docx", entity_display_name: "Imobiliária Lda",
                        reference_display: "LEASE-22", amount_display: nil, ai_confidence_score: 0.44,
                        filename: "lease_agreement.docx"),
      document_types: TYPES, class: "max-w-md"
    )
  end
end
