require "rails_helper"

RSpec.describe Document, type: :model do
  let(:workspace) { create(:workspace) }

  describe "associations" do
    it { is_expected.to belong_to(:reviewed_by).class_name("User").optional }
    it { is_expected.to belong_to(:classification).class_name("DocumentType").with_foreign_key(:document_type_id).optional }
  end

  describe "validations" do
    it { is_expected.to validate_presence_of(:document_type) }
    it { is_expected.to validate_presence_of(:ai_status) }
    it { is_expected.to validate_presence_of(:review_status) }
    it { is_expected.to validate_presence_of(:source) }
  end

  describe "enums" do
    it {
      is_expected.to define_enum_for(:document_type)
        .with_values(expense_invoice: 0, revenue_invoice: 1, bank_statement: 2, receipt: 3, other: 4, insurance_policy: 5, vehicle_document: 6, contract: 7, certificate: 8, tax_document: 9, identification: 10, proposal: 11, correspondence: 12, bank_journal_entry: 13, credit_note: 14)
    }

    it {
      is_expected.to define_enum_for(:ai_status)
        .with_values(pending: 0, processing: 1, completed: 2, failed: 3, skipped: 4)
        .with_prefix(:ai)
    }

    it {
      is_expected.to define_enum_for(:review_status)
        .with_values(pending: 0, approved: 1, rejected: 2)
        .with_prefix(:review)
    }

    it {
      is_expected.to define_enum_for(:source)
        .with_values(manual_upload: 0, email: 1, notion: 2, sent_email: 3)
    }
  end

  describe "scopes" do
    describe ".for_month" do
      it "returns documents for the given month" do
        jan_doc = create(:document, document_date: Date.new(2025, 1, 15))
        feb_doc = create(:document, document_date: Date.new(2025, 2, 10))

        expect(Document.for_month(2025, 1)).to include(jan_doc)
        expect(Document.for_month(2025, 1)).not_to include(feb_doc)
      end

      it "includes documents on the first and last day of the month" do
        first = create(:document, document_date: Date.new(2025, 3, 1))
        last  = create(:document, document_date: Date.new(2025, 3, 31))
        prev  = create(:document, document_date: Date.new(2025, 2, 28))
        next_ = create(:document, document_date: Date.new(2025, 4, 1))

        result = Document.for_month(2025, 3)
        expect(result).to include(first, last)
        expect(result).not_to include(prev, next_)
      end

      it "does not crash when a document has nil or blank document_date in metadata" do
        nil_date = create(:document, document_date: nil)
        expect { Document.for_month(2025, 1).to_a }.not_to raise_error
        expect(Document.for_month(2025, 1)).not_to include(nil_date)
      end
    end

    describe ".document_date_from" do
      it "is inclusive on the boundary date" do
        on_date    = create(:document, document_date: Date.new(2026, 6, 1))
        before_it  = create(:document, document_date: Date.new(2026, 5, 31))

        result = Document.document_date_from(Date.new(2026, 6, 1))
        expect(result).to include(on_date)
        expect(result).not_to include(before_it)
      end

      it "does not crash on a document with nil document_date" do
        nil_date = create(:document, document_date: nil)
        expect { Document.document_date_from(Date.new(2026, 1, 1)).to_a }.not_to raise_error
        expect(Document.document_date_from(Date.new(2026, 1, 1))).not_to include(nil_date)
      end
    end

    describe ".document_date_to" do
      it "is inclusive on the boundary date" do
        on_date   = create(:document, document_date: Date.new(2026, 12, 31))
        after_it  = create(:document, document_date: Date.new(2027, 1, 1))

        result = Document.document_date_to(Date.new(2026, 12, 31))
        expect(result).to include(on_date)
        expect(result).not_to include(after_it)
      end
    end

    describe ".amount_at_least" do
      it "returns only documents with amount >= threshold" do
        cheap  = create(:document, amount_cents: 500)
        pricey = create(:document, amount_cents: 20_000)

        result = Document.amount_at_least(1_000)
        expect(result).to include(pricey)
        expect(result).not_to include(cheap)
      end

      it "does not crash on a document with nil amount_cents" do
        nil_amount = create(:document, amount_cents: nil, tax_amount_cents: nil)
        expect { Document.amount_at_least(100).to_a }.not_to raise_error
        expect(Document.amount_at_least(100)).not_to include(nil_amount)
      end

      it "does not crash on a document with non-numeric amount_cents in metadata" do
        bad = create(:document, amount_cents: nil, tax_amount_cents: nil)
        bad.update_columns(metadata: (bad.metadata || {}).merge("amount_cents" => "not-a-number"))
        expect { Document.amount_at_least(100).to_a }.not_to raise_error
      end
    end

    describe ".amount_at_most" do
      it "returns only documents with amount <= threshold" do
        cheap  = create(:document, amount_cents: 500)
        pricey = create(:document, amount_cents: 20_000)

        result = Document.amount_at_most(1_000)
        expect(result).to include(cheap)
        expect(result).not_to include(pricey)
      end
    end

    describe ".by_entity" do
      it "matches vendor_name" do
        doc   = create(:document, vendor_name: "Acme Corp")
        other = create(:document, vendor_name: "Globex Inc")

        expect(Document.by_entity([ "Acme" ])).to include(doc)
        expect(Document.by_entity([ "Acme" ])).not_to include(other)
      end

      it "matches client_name" do
        doc = create(:document, :revenue_invoice, client_name: "Beta Ltd")
        expect(Document.by_entity([ "Beta" ])).to include(doc)
      end

      it "matches sender_name" do
        doc = create(:document, sender_name: "SenderCo")
        doc.update_columns(metadata: (doc.metadata || {}).merge("sender_name" => "SenderCo", "vendor_name" => nil))
        expect(Document.by_entity([ "SenderCo" ])).to include(doc)
      end

      it "is case-insensitive (ILIKE)" do
        doc = create(:document, vendor_name: "UPPER NAME")
        expect(Document.by_entity([ "upper name" ])).to include(doc)
      end

      it "OR's multiple terms" do
        doc_a = create(:document, vendor_name: "Alpha")
        doc_b = create(:document, vendor_name: "Beta")
        other = create(:document, vendor_name: "Gamma")

        result = Document.by_entity(%w[Alpha Beta])
        expect(result).to include(doc_a, doc_b)
        expect(result).not_to include(other)
      end

      it "returns all when terms is empty" do
        doc = create(:document)
        expect(Document.by_entity([])).to include(doc)
      end
    end

    describe ".by_reference" do
      it "matches invoice_number" do
        doc   = create(:document, invoice_number: "FT2024/001")
        other = create(:document, invoice_number: "RC2024/002")

        expect(Document.by_reference([ "FT2024" ])).to include(doc)
        expect(Document.by_reference([ "FT2024" ])).not_to include(other)
      end

      it "matches receipt_number" do
        doc = create(:document, :receipt, receipt_number: "RC2024/007")
        expect(Document.by_reference([ "RC2024/007" ])).to include(doc)
      end
    end

    describe ".by_expense_category" do
      it "matches documents with the given expense_category in metadata" do
        travel = create(:document, expense_category: "travel")
        meals  = create(:document, expense_category: "meals")

        expect(Document.by_expense_category([ "travel" ])).to include(travel)
        expect(Document.by_expense_category([ "travel" ])).not_to include(meals)
      end

      it "filters out invalid categories (not in EXPENSE_CATEGORIES)" do
        doc = create(:document, expense_category: "travel")
        # 'nonsense' is not in EXPENSE_CATEGORIES — scope returns none
        expect(Document.by_expense_category([ "nonsense" ])).to be_empty
        expect(Document.by_expense_category([ "travel", "nonsense" ])).to include(doc)
      end
    end

    describe ".needs_review" do
      it "includes AI-completed docs awaiting sign-off; excludes approved, rejected and AI-failed" do
        review_doc = create(:document, :in_review)
        approved_doc = create(:document, :approved)
        rejected_doc = create(:document, :rejected)
        failed_doc = create(:document, :ai_failed)

        expect(Document.needs_review).to include(review_doc)
        expect(Document.needs_review).not_to include(approved_doc, rejected_doc, failed_doc)
      end
    end

    describe ".ai_failed_attention" do
      it "returns only AI-failed documents" do
        failed_doc = create(:document, :ai_failed)
        review_doc = create(:document, :in_review)

        expect(Document.ai_failed_attention).to include(failed_doc)
        expect(Document.ai_failed_attention).not_to include(review_doc)
      end
    end

    describe ".by_organization" do
      let(:user) { create(:user, workspace: workspace) }
      let(:org) { create(:organization, workspace: workspace) }
      let(:person) { create(:person, workspace: workspace) }
      let(:contact) { create(:contact, workspace: workspace, person: person) }

      before { create(:organization_membership, organization: org, person: person, status: :active) }

      # A document linked (via document_email_messages) to an email that arrived
      # in `account`, sent by the org member's contact.
      def org_document_in(account)
        message = create(:email_message, email_account: account, contact: contact)
        document = create(:document, workspace: workspace, source: :email)
        document.email_messages << message
        document
      end

      it "returns documents tied to the organization's members" do
        document = org_document_in(create(:email_account, workspace: workspace))
        expect(Document.by_organization(org)).to include(document)
      end

      it "hides documents from email accounts the user cannot read (per-user sharing)" do
        readable = create(:email_account, workspace: workspace)
        create(:email_account_user, email_account: readable, user: user, can_read: true)
        hidden = create(:email_account, workspace: workspace) # never shared with the user

        visible_doc = org_document_in(readable)
        hidden_doc  = org_document_in(hidden)

        scoped = Document.by_organization(org, accessible_to: user)
        expect(scoped).to include(visible_doc)
        expect(scoped).not_to include(hidden_doc)
      end
    end
  end

  describe "review sign-off" do
    let(:workspace) { create(:workspace) }
    let(:reviewer) { create(:user, workspace: workspace) }
    let(:new_type) { DocumentType.create!(workspace: workspace, name: "receipt", color: "#000", prompt: "test") }

    describe "#approve!" do
      it "marks the document review-approved (leaving the AI axis) and records the reviewer" do
        doc = create(:document, :in_review, workspace: workspace)

        doc.approve!(by: reviewer)

        expect(doc).to be_review_approved
        expect(doc).to be_ai_completed
        expect(doc.reviewed_by).to eq(reviewer)
        expect(doc.reviewed_at).to be_present
        expect(Document.needs_review).not_to include(doc)
      end
    end

    describe "#reclassify!" do
      it "re-files under the new type and signs it off (review-approved, out of the queue)" do
        doc = create(:document, :in_review, workspace: workspace)

        doc.reclassify!(new_type, by: reviewer)

        expect(doc.document_type_id).to eq(new_type.id)
        expect(doc).to be_review_approved
        expect(doc.reviewed_by).to eq(reviewer)
        expect(Document.needs_review).not_to include(doc)
      end
    end

    describe "#reject!" do
      it "marks the document review-rejected and drops it from the review queue" do
        doc = create(:document, :in_review, workspace: workspace)

        doc.reject!

        expect(doc.reload).to be_review_rejected
        expect(Document.needs_review).not_to include(doc)
      end
    end

    describe "#restore!" do
      it "returns an approved doc to the review queue and clears the reviewer stamps" do
        doc = create(:document, :approved, workspace: workspace, reviewed_by: reviewer, reviewed_at: Time.current)

        doc.restore!

        expect(doc).to be_review_pending
        expect(doc.reviewed_by).to be_nil
        expect(doc.reviewed_at).to be_nil
        expect(Document.needs_review).to include(doc)
      end
    end
  end

  describe "#display_status" do
    it "prefers the active AI lifecycle, otherwise the review state" do
      expect(build(:document, ai_status: :processing).display_status).to eq("processing")
      expect(build(:document, :ai_failed).display_status).to eq("failed")
      expect(build(:document, :in_review).display_status).to eq("pending")
      expect(build(:document, :approved).display_status).to eq("approved")
      expect(build(:document, :rejected).display_status).to eq("rejected")
    end
  end

  describe "#image?" do
    it "returns true for image content types" do
      doc = build(:document)
      doc.original_file.attach(io: StringIO.new("img"), filename: "test.jpg", content_type: "image/jpeg")
      expect(doc.image?).to be true
    end
  end

  describe "#pdf?" do
    it "returns true for PDF content types" do
      doc = build(:document)
      doc.original_file.attach(io: StringIO.new("pdf"), filename: "test.pdf", content_type: "application/pdf")
      expect(doc.pdf?).to be true
    end
  end

  describe "#thumbnailable?" do
    it "is true for PDFs and images, false for other types" do
      doc = build(:document) # factory attaches a PDF
      expect(doc.thumbnailable?).to be true

      doc.original_file.attach(io: StringIO.new("png"), filename: "scan.png", content_type: "image/png")
      expect(doc.thumbnailable?).to be true

      doc.original_file.attach(io: StringIO.new("zip"), filename: "archive.zip", content_type: "application/zip")
      expect(doc.thumbnailable?).to be false
    end
  end

  describe "thumbnail generation enqueue" do
    it "enqueues a GenerateThumbnailJob when a thumbnailable document is created" do
      expect { create(:document) }.to have_enqueued_job(Documents::GenerateThumbnailJob)
    end

    it "does not enqueue for a non-thumbnailable type" do
      doc = build(:document)
      doc.original_file.attach(io: StringIO.new("zip"), filename: "archive.zip", content_type: "application/zip")
      expect { doc.save! }.not_to have_enqueued_job(Documents::GenerateThumbnailJob)
    end
  end

  describe "#entity_display_name" do
    it "returns vendor_name for expense invoices" do
      dt = DocumentType.create!(workspace: workspace, name: "expense_invoice", color: "#000", prompt: "test")
      doc = build(:document, document_type: :expense_invoice, document_type_id: dt.id, vendor_name: "ACME Corp")
      expect(doc.entity_display_name).to eq("ACME Corp")
    end

    it "returns client_name for revenue invoices" do
      dt = DocumentType.create!(workspace: workspace, name: "revenue_invoice", color: "#000", prompt: "test")
      doc = build(:document, :revenue_invoice, document_type_id: dt.id, client_name: "Client Ltd")
      expect(doc.entity_display_name).to eq("Client Ltd")
    end

    it "returns bank_name for bank statements" do
      dt = DocumentType.create!(workspace: workspace, name: "bank_statement", color: "#000", prompt: "test")
      doc = build(:document, :bank_statement, document_type_id: dt.id, bank_name: "Millennium BCP")
      expect(doc.entity_display_name).to eq("Millennium BCP")
    end

    it "returns vendor_name for receipts" do
      dt = DocumentType.create!(workspace: workspace, name: "receipt", color: "#000", prompt: "test")
      doc = build(:document, :receipt, document_type_id: dt.id, vendor_name: "Shop ABC")
      expect(doc.entity_display_name).to eq("Shop ABC")
    end

    it "returns vendor_name for other documents" do
      dt = DocumentType.create!(workspace: workspace, name: "other", color: "#000", prompt: "test")
      doc = build(:document, :other, document_type_id: dt.id, vendor_name: "Some Entity")
      expect(doc.entity_display_name).to eq("Some Entity")
    end
  end

  describe "#reference_display" do
    it "returns invoice_number for expense invoices" do
      dt = DocumentType.create!(workspace: workspace, name: "expense_invoice", color: "#000", prompt: "test")
      doc = build(:document, document_type: :expense_invoice, document_type_id: dt.id, invoice_number: "FT2025/001")
      expect(doc.reference_display).to eq("FT2025/001")
    end

    it "returns invoice_number for revenue invoices" do
      dt = DocumentType.create!(workspace: workspace, name: "revenue_invoice", color: "#000", prompt: "test")
      doc = build(:document, :revenue_invoice, document_type_id: dt.id, invoice_number: "FT2025/002")
      expect(doc.reference_display).to eq("FT2025/002")
    end

    it "returns period range for bank statements" do
      dt = DocumentType.create!(workspace: workspace, name: "bank_statement", color: "#000", prompt: "test")
      doc = build(:document, :bank_statement, document_type_id: dt.id,
        period_start: Date.new(2025, 1, 1),
        period_end: Date.new(2025, 1, 31))
      expect(doc.reference_display).to eq("01/01/2025 - 31/01/2025")
    end

    it "returns nil for bank statements without period" do
      dt = DocumentType.create!(workspace: workspace, name: "bank_statement", color: "#000", prompt: "test")
      doc = build(:document, :bank_statement, document_type_id: dt.id, period_start: nil, period_end: nil)
      expect(doc.reference_display).to be_nil
    end

    it "returns receipt_number for receipts" do
      dt = DocumentType.create!(workspace: workspace, name: "receipt", color: "#000", prompt: "test")
      doc = build(:document, :receipt, document_type_id: dt.id, receipt_number: "RC2025/001")
      expect(doc.reference_display).to eq("RC2025/001")
    end

    it "returns nil for other documents" do
      dt = DocumentType.create!(workspace: workspace, name: "other", color: "#000", prompt: "test")
      doc = build(:document, :other, document_type_id: dt.id)
      expect(doc.reference_display).to be_nil
    end
  end

  # ── Document#searchable_fields_changed? ─────────────────────────────────────

  describe "#searchable_fields_changed?" do
    it "returns true after vendor_name changes via extracted field" do
      doc = create(:document, vendor_name: "Old Vendor")
      doc.update!(vendor_name: "New Vendor")
      expect(doc.searchable_fields_changed?).to be true
    end

    it "returns true after client_name changes via extracted field" do
      doc = create(:document, :revenue_invoice, client_name: "Old Client")
      doc.update!(client_name: "New Client")
      expect(doc.searchable_fields_changed?).to be true
    end

    it "returns true after description changes" do
      doc = create(:document, description: "old")
      doc.update!(description: "new")
      expect(doc.searchable_fields_changed?).to be true
    end

    it "returns true after review_status changes" do
      doc = create(:document, :in_review)
      doc.update!(review_status: :approved)
      expect(doc.searchable_fields_changed?).to be true
    end

    it "returns false when no tracked field changed" do
      doc = create(:document, starred: false)
      doc.update!(starred: true)
      expect(doc.searchable_fields_changed?).to be false
    end
  end

  # ── Document#nif_status ──────────────────────────────────────────────────────

  describe "#nif_status" do
    let(:doc) { build(:document, document_type: :expense_invoice) }

    context "when company_nif is blank" do
      it "returns nil" do
        expect(doc.nif_status(nil)).to be_nil
        expect(doc.nif_status("")).to be_nil
      end
    end

    context "for non-applicable document types" do
      it "returns nil for bank_statement" do
        doc.document_type = :bank_statement
        expect(doc.nif_status("PT123456789")).to be_nil
      end

      it "returns nil for revenue_invoice" do
        doc.document_type = :revenue_invoice
        expect(doc.nif_status("PT123456789")).to be_nil
      end
    end

    context "when company_vat_present is true (EU-standard VAT on document)" do
      it "returns :ok without inspecting vendor_nif" do
        doc.company_vat_present = true
        expect(doc.nif_status("PT123456789")).to eq(:ok)
      end
    end

    context "when buyer_nif matches" do
      it "returns :ok for exact ISO match" do
        doc.buyer_nif = "PT123456789"
        expect(doc.nif_status("PT123456789")).to eq(:ok)
      end

      it "normalizes by stripping PT prefix and spaces" do
        doc.buyer_nif = "PT 123 456 789"
        expect(doc.nif_status("123456789")).to eq(:ok)
      end

      it "normalizes dots and dashes" do
        doc.buyer_nif = "123.456.789"
        expect(doc.nif_status("123456789")).to eq(:ok)
      end
    end

    context "when buyer_nif is present but mismatches" do
      it "returns :mismatch" do
        doc.buyer_nif = "PT987654321"
        expect(doc.nif_status("PT123456789")).to eq(:mismatch)
      end
    end

    context "when buyer_nif is blank" do
      it "returns :missing when company_vat_present is false" do
        doc.buyer_nif         = nil
        doc.company_vat_present = false
        expect(doc.nif_status("PT123456789")).to eq(:missing)
      end
    end
  end
end
