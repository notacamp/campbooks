# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::ExtractedFields do
  # Build a minimal in-memory Document without hitting the factory chain —
  # enough to test read/write/dirty behaviour without DB overhead.
  def new_doc(**attrs)
    Document.new(
      document_type: :expense_invoice,
      ai_status: :pending,
      review_status: :pending,
      source: :manual_upload,
      **attrs
    )
  end

  # ── String fields ────────────────────────────────────────────────────────────

  describe "string fields" do
    it "round-trips vendor_name through metadata" do
      doc = new_doc
      doc.vendor_name = "  Acme Lda  "
      expect(doc.vendor_name).to eq("Acme Lda")
      expect(doc.metadata["vendor_name"]).to eq("Acme Lda")
    end

    it "deletes the key when assigned nil" do
      doc = new_doc
      doc.vendor_name = "Acme"
      doc.vendor_name = nil
      expect(doc.metadata).not_to have_key("vendor_name")
    end

    it "deletes the key when assigned blank string" do
      doc = new_doc
      doc.vendor_name = "Acme"
      doc.vendor_name = "  "
      expect(doc.metadata).not_to have_key("vendor_name")
    end

    it "is nil-safe when metadata is nil" do
      doc = new_doc(metadata: nil)
      expect(doc.vendor_name).to be_nil
    end
  end

  # ── Integer (cents) fields ───────────────────────────────────────────────────

  describe "integer (cents) fields" do
    it "round-trips amount_cents through metadata" do
      doc = new_doc
      doc.amount_cents = 12_345
      expect(doc.amount_cents).to eq(12_345)
    end

    it "coerces '12345.0' string to integer" do
      doc = new_doc
      doc.amount_cents = "12345.0"
      expect(doc.amount_cents).to eq(12_345)
    end

    it "deletes key when assigned nil" do
      doc = new_doc
      doc.amount_cents = 100
      doc.amount_cents = nil
      expect(doc.metadata).not_to have_key("amount_cents")
    end
  end

  # ── Number field ─────────────────────────────────────────────────────────────

  describe "number fields" do
    it "round-trips tax_rate as Float" do
      doc = new_doc
      doc.tax_rate = "23.5"
      expect(doc.tax_rate).to eq(23.5)
    end

    it "deletes key for blank input" do
      doc = new_doc
      doc.tax_rate = 23.5
      doc.tax_rate = nil
      expect(doc.metadata).not_to have_key("tax_rate")
    end
  end

  # ── Date fields ──────────────────────────────────────────────────────────────

  describe "date fields" do
    it "stores date as ISO-8601 string in metadata" do
      doc = new_doc
      doc.document_date = Date.new(2024, 3, 15)
      expect(doc.metadata["document_date"]).to eq("2024-03-15")
    end

    it "reads back as a Date object" do
      doc = new_doc
      doc.document_date = "2024-03-15"
      expect(doc.document_date).to eq(Date.new(2024, 3, 15))
    end

    it "accepts parseable strings" do
      doc = new_doc
      doc.document_date = "15/03/2024"
      expect(doc.metadata["document_date"]).to eq("2024-03-15")
    end

    it "deletes key for nil" do
      doc = new_doc
      doc.document_date = Date.today
      doc.document_date = nil
      expect(doc.metadata).not_to have_key("document_date")
    end
  end

  # ── Boolean field ────────────────────────────────────────────────────────────

  describe "boolean fields" do
    it "stores true as true in metadata" do
      doc = new_doc
      doc.company_vat_present = true
      expect(doc.company_vat_present).to eq(true)
      expect(doc.company_vat_present?).to eq(true)
    end

    it "stores false as false" do
      doc = new_doc
      doc.company_vat_present = false
      expect(doc.company_vat_present).to eq(false)
    end

    it "coerces '1' to true" do
      doc = new_doc
      doc.company_vat_present = "1"
      expect(doc.company_vat_present).to eq(true)
    end

    it "coerces '0' to false" do
      doc = new_doc
      doc.company_vat_present = "0"
      expect(doc.company_vat_present).to eq(false)
    end

    it "deletes key for nil" do
      doc = new_doc
      doc.company_vat_present = true
      doc.company_vat_present = nil
      expect(doc.metadata).not_to have_key("company_vat_present")
    end

    it "predicate returns false when field is nil" do
      doc = new_doc
      expect(doc.company_vat_present?).to eq(false)
    end
  end

  # ── Currency (EUR default) ───────────────────────────────────────────────────

  describe "currency" do
    it "defaults to 'EUR' when not set" do
      doc = new_doc
      expect(doc.currency).to eq("EUR")
    end

    it "returns the stored value when set" do
      doc = new_doc
      doc.currency = "USD"
      expect(doc.currency).to eq("USD")
    end
  end

  # ── expense_category (legacy int mapping + enum validation) ──────────────────

  describe "expense_category" do
    it "accepts valid string category" do
      doc = new_doc
      doc.expense_category = "travel"
      expect(doc.expense_category).to eq("travel")
    end

    it "maps legacy integer 0 to 'travel'" do
      doc = new_doc
      doc.expense_category = 0
      expect(doc.expense_category).to eq("travel")
    end

    it "maps legacy integer 9 to 'other'" do
      doc = new_doc
      doc.expense_category = 9
      expect(doc.expense_category).to eq("other")
    end

    it "maps legacy string integer '5' to 'software'" do
      doc = new_doc
      doc.expense_category = "5"
      expect(doc.expense_category).to eq("software")
    end

    it "returns nil for value not in EXPENSE_CATEGORIES" do
      doc = new_doc
      doc.expense_category = "nonexistent"
      expect(doc.expense_category).to be_nil
      expect(doc.metadata).not_to have_key("expense_category")
    end

    it "deletes key for nil" do
      doc = new_doc
      doc.expense_category = "travel"
      doc.expense_category = nil
      expect(doc.metadata).not_to have_key("expense_category")
    end
  end

  # ── Money readers ────────────────────────────────────────────────────────────

  describe "money readers" do
    it "returns Money object for amount" do
      doc = new_doc
      doc.amount_cents = 12_345
      money = doc.amount
      expect(money).to be_a(Money)
      expect(money.cents).to eq(12_345)
      expect(money.currency.iso_code).to eq("EUR")
    end

    it "returns nil for amount when cents is nil" do
      doc = new_doc
      expect(doc.amount).to be_nil
    end

    it "uses the document's currency, not hardcoded EUR" do
      doc = new_doc
      doc.amount_cents = 5_000
      doc.currency = "USD"
      expect(doc.amount.currency.iso_code).to eq("USD")
    end

    it "returns Money objects for tax_amount, opening_balance, closing_balance" do
      doc = new_doc
      doc.tax_amount_cents = 2_300
      doc.opening_balance_cents = 100_000
      doc.closing_balance_cents = 120_000

      expect(doc.tax_amount.cents).to eq(2_300)
      expect(doc.opening_balance.cents).to eq(100_000)
      expect(doc.closing_balance.cents).to eq(120_000)
    end
  end

  # ── Metadata nil-safety ──────────────────────────────────────────────────────

  describe "metadata nil-safety" do
    it "initializes metadata to {} when writing the first field" do
      doc = Document.new(document_type: :expense_invoice, source: :manual_upload,
                         ai_status: :pending, review_status: :pending)
      # metadata is nil before any write
      doc.vendor_name = "Test"
      expect(doc.metadata).to eq({ "vendor_name" => "Test" })
    end
  end

  # ── Dirty tracking ───────────────────────────────────────────────────────────

  describe "dirty tracking" do
    it "marks the document as changed when a field is written" do
      doc = new_doc
      expect { doc.vendor_name = "Acme" }.to change { doc.changed? }.to(true)
    end

    it "the changed attribute is metadata (not the legacy column name)" do
      doc = new_doc
      doc.vendor_name = "Acme"
      expect(doc.changes).to have_key("metadata")
    end
  end

  # ── saved_change_to_extracted_field? ─────────────────────────────────────────

  describe "#saved_change_to_extracted_field?" do
    # Stub searchable_fields_changed? because it still calls the now-removed
    # saved_change_to_vendor_name? AR generated method. That call site in
    # document.rb is out of scope; other agents will fix it. The stub isolates
    # the concern's own method from the breakage.
    before do
      allow_any_instance_of(Document).to receive(:searchable_fields_changed?).and_return(false)
    end

    it "returns true when the field's value changed since last save" do
      doc = create(:document, vendor_name: "Before")
      doc.vendor_name = "After"
      doc.save!

      expect(doc.saved_change_to_extracted_field?("vendor_name")).to be true
    end

    it "returns false when the field's value did not change" do
      doc = create(:document, vendor_name: "Same")
      doc.vendor_name = "Same"
      doc.save!

      expect(doc.saved_change_to_extracted_field?("vendor_name")).to be false
    end

    it "returns false when no save has occurred" do
      doc = new_doc
      doc.vendor_name = "Acme"
      expect(doc.saved_change_to_extracted_field?("vendor_name")).to be false
    end
  end

  # ── .extracted_field_names ───────────────────────────────────────────────────

  describe ".extracted_field_names" do
    it "includes all 23 declared fields" do
      names = Document.extracted_field_names
      expect(names).to include("vendor_name", "amount_cents", "document_date",
                                "tax_rate", "expense_category", "company_vat_present",
                                "currency", "payment_method")
      expect(names.size).to eq(23)
    end
  end
end
