# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::ExtractedFieldSet do
  # Plain stub so the field set is tested in isolation (no DB), mirroring how
  # SkimBuilder is exercised. Responds to any column like an AR record would,
  # returning the configured value (nil when unset).
  Classification = Struct.new(:extraction_schema)

  class StubDoc
    def initialize(document_type:, schema: nil, metadata: {}, columns: {})
      @document_type = document_type
      @schema = schema
      @metadata = metadata
      @columns = columns
    end

    attr_reader :document_type, :metadata

    def classification
      @schema.nil? ? nil : Classification.new(@schema)
    end

    def respond_to_missing?(_name, _include_private = false) = true
    def method_missing(name, *) = @columns[name]
  end

  it "built-in type returns its full column field set, even when blank" do
    doc = StubDoc.new(document_type: "expense_invoice", columns: { vendor_name: "Acme Lda", amount_cents: 12_345 })
    fields = described_class.new(doc).fields

    expect(fields.size).to eq(11)
    expect(fields.all? { |f| f[:store] == :column }).to be_truthy

    vendor = fields.find { |f| f[:key] == "vendor_name" }
    expect(vendor[:value]).to eq("Acme Lda")
    expect(vendor[:kind]).to eq(:text)

    amount = fields.find { |f| f[:key] == "amount_cents" }
    expect(amount[:value]).to eq(12_345)
    expect(amount[:kind]).to eq(:money)

    # A field with no column or metadata value is still present (blank kept).
    expect(fields.map { |f| f[:key] }).to include("buyer_nif")
    expect(fields.find { |f| f[:key] == "buyer_nif" }[:value]).to be_nil

    # Enum fields are present for display...
    category = fields.find { |f| f[:key] == "expense_category" }
    expect(category[:kind]).to eq(:enum_expense_category)
  end

  it "built-in type falls back to metadata when the column is blank (legacy rows)" do
    doc = StubDoc.new(document_type: "expense_invoice",
                      columns: { vendor_name: nil },
                      metadata: { "vendor_name" => "From Metadata" })
    vendor = described_class.new(doc).fields.find { |f| f[:key] == "vendor_name" }

    expect(vendor[:value]).to eq("From Metadata")
  end

  it "custom type with a Hash extraction_schema uses schema fields, edited in metadata" do
    schema = { "policy_no" => { "description" => "Policy number" }, "insurer" => {} }
    doc = StubDoc.new(document_type: "insurance_policy", schema: schema,
                      metadata: { "policy_no" => "AX-200" })
    fields = described_class.new(doc).fields

    expect(fields.map { |f| f[:key] }).to eq(%w[policy_no insurer])
    expect(fields.all? { |f| f[:store] == :metadata }).to be_truthy
    expect(fields.first[:label]).to eq("Policy number") # schema description wins
    expect(fields.last[:label]).to eq("Insurer")        # humanised fallback
    expect(fields.first[:value]).to eq("AX-200")
  end

  it "unknown type without a schema surfaces raw metadata, minus the title" do
    doc = StubDoc.new(document_type: "mystery",
                      metadata: { "title" => "Ignore me", "reference" => "R-1" })
    fields = described_class.new(doc).fields

    expect(fields.map { |f| f[:key] }).to eq(%w[reference])
    expect(fields.first[:value]).to eq("R-1")
    expect(fields.first[:store]).to eq(:metadata)
  end

  it "COLUMN_KEYS lists every editable column; ENUM_KEYS flags the enum-backed ones" do
    expect(described_class::COLUMN_KEYS).to include(:vendor_name)
    expect(described_class::COLUMN_KEYS).to include(:amount_cents)
    # Enums are editable too (as <select>s), so they're in COLUMN_KEYS...
    expect(described_class::COLUMN_KEYS).to include(:expense_category)
    expect(described_class::COLUMN_KEYS).to include(:payment_method)
    # ...and ENUM_KEYS isolates them for the controller's blank -> nil coercion.
    expect(described_class::ENUM_KEYS.sort).to eq(%i[expense_category payment_method].sort)
  end
end
