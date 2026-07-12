# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::ExtractedFieldSet do
  # Minimal stub that responds like a Document (no DB).
  Classification = Struct.new(:extraction_schema) unless defined?(Classification)

  class StubDoc
    def initialize(document_type:, schema: nil, metadata: {})
      @document_type = document_type
      @schema        = schema
      @metadata      = metadata
    end

    attr_reader :document_type, :metadata

    def classification
      @schema.nil? ? nil : Classification.new(@schema)
    end
  end

  # ── Schema-driven fields (enriched schema) ───────────────────────────────────

  describe "schema-driven fields from an enriched schema" do
    let(:schema) do
      {
        "vendor_name"   => { "type" => "string", "description" => "Vendor Name",  "position" => 1 },
        "amount_cents"  => { "type" => "money",  "description" => "Amount",       "position" => 2 },
        "document_date" => { "type" => "date",   "description" => "Document Date", "position" => 3 },
        "status"        => { "type" => "enum",   "description" => "Status",
                             "values" => %w[open closed],                          "position" => 4 }
      }
    end

    let(:doc) do
      StubDoc.new(
        document_type: "insurance_policy",
        schema: schema,
        metadata: {
          "vendor_name"   => "Insurer Co",
          "amount_cents"  => 50_000,
          "document_date" => "2024-06-01",
          "status"        => "open"
        }
      )
    end

    subject(:fields) { described_class.new(doc).fields }

    it "returns fields in schema position order" do
      expect(fields.map { |f| f[:key] }).to eq(%w[vendor_name amount_cents document_date status])
    end

    it "resolves values via Field#read (typed)" do
      date_field = fields.find { |f| f[:key] == "document_date" }
      expect(date_field[:value]).to eq(Date.new(2024, 6, 1))

      money_field = fields.find { |f| f[:key] == "amount_cents" }
      expect(money_field[:value]).to eq(50_000)
    end

    it "sets store to :metadata for all fields" do
      expect(fields.all? { |f| f[:store] == :metadata }).to be true
    end

    it "sets kind correctly per type" do
      expect(fields.find { |f| f[:key] == "amount_cents" }[:kind]).to  eq(:money)
      expect(fields.find { |f| f[:key] == "document_date" }[:kind]).to eq(:date)
      expect(fields.find { |f| f[:key] == "vendor_name" }[:kind]).to   eq(:text)
      expect(fields.find { |f| f[:key] == "status" }[:kind]).to        eq(:enum)
    end

    it "includes enum_values for enum fields and nil for others" do
      status_f  = fields.find { |f| f[:key] == "status" }
      vendor_f  = fields.find { |f| f[:key] == "vendor_name" }
      expect(status_f[:enum_values]).to  eq(%w[open closed])
      expect(vendor_f[:enum_values]).to  be_nil
    end
  end

  # ── Schema-driven fields (old-format schema without label_key/position) ──────

  describe "schema-driven fields from an old-format schema" do
    let(:doc) do
      StubDoc.new(
        document_type: "insurance_policy",
        schema: {
          "policy_no" => { "type" => "string", "description" => "Policy number" },
          "insurer"   => {}
        },
        metadata: { "policy_no" => "AX-200" }
      )
    end

    subject(:fields) { described_class.new(doc).fields }

    it "returns fields in insertion order" do
      expect(fields.map { |f| f[:key] }).to eq(%w[policy_no insurer])
    end

    it "uses description as label when label_key is absent" do
      expect(fields.first[:label]).to eq("Policy number")
    end

    it "humanizes the key when no description or label_key" do
      expect(fields.last[:label]).to eq("Insurer")
    end

    it "reads value from metadata" do
      expect(fields.first[:value]).to eq("AX-200")
    end
  end

  # ── never_blank fallback ─────────────────────────────────────────────────────

  describe "never_blank fallback" do
    it "falls back to raw metadata when schema fields are all blank" do
      doc = StubDoc.new(
        document_type: "insurance_policy",
        schema: { "policy_no" => { "type" => "string", "description" => "Policy number" } },
        metadata: { "flight_number" => "TP451", "gate" => "A12" }
      )
      fields = described_class.new(doc).fields
      # schema field is blank, so falls back to metadata
      expect(fields.map { |f| f[:key] }).to match_array(%w[flight_number gate])
    end
  end

  # ── No classification / no schema → raw metadata ─────────────────────────────

  describe "no classification" do
    it "surfaces raw metadata keys (minus title) when no schema" do
      doc = StubDoc.new(
        document_type: "mystery",
        schema: nil,
        metadata: { "title" => "Ignored", "reference" => "R-1", "note" => "See me" }
      )
      fields = described_class.new(doc).fields
      expect(fields.map { |f| f[:key] }).to match_array(%w[reference note])
      expect(fields.first[:store]).to eq(:metadata)
    end

    it "returns empty array for a document with no metadata and no schema" do
      doc = StubDoc.new(document_type: "other", schema: nil, metadata: {})
      expect(described_class.new(doc).fields).to eq([])
    end
  end

  # ── Constants that were REMOVED ──────────────────────────────────────────────

  describe "removed constants" do
    it "does not define COLUMN_KEYS" do
      expect(described_class.const_defined?(:COLUMN_KEYS)).to be false
    end

    it "does not define ENUM_KEYS" do
      expect(described_class.const_defined?(:ENUM_KEYS)).to be false
    end

    it "does not define TYPE_FIELDS" do
      expect(described_class.const_defined?(:TYPE_FIELDS)).to be false
    end
  end
end
