# frozen_string_literal: true

require "test_helper"

module Documents
  class ExtractedFieldSetTest < ActiveSupport::TestCase
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

    test "built-in type returns its full column field set, even when blank" do
      doc = StubDoc.new(document_type: "expense_invoice", columns: { vendor_name: "Acme Lda", amount_cents: 12_345 })
      fields = ExtractedFieldSet.new(doc).fields

      assert_equal 11, fields.size
      assert(fields.all? { |f| f[:store] == :column })

      vendor = fields.find { |f| f[:key] == "vendor_name" }
      assert_equal "Acme Lda", vendor[:value]
      assert_equal :text, vendor[:kind]

      amount = fields.find { |f| f[:key] == "amount_cents" }
      assert_equal 12_345, amount[:value]
      assert_equal :money, amount[:kind]

      # A field with no column or metadata value is still present (blank kept).
      assert_includes fields.map { |f| f[:key] }, "buyer_nif"
      assert_nil fields.find { |f| f[:key] == "buyer_nif" }[:value]

      # Enum fields are present for display...
      category = fields.find { |f| f[:key] == "expense_category" }
      assert_equal :enum_expense_category, category[:kind]
    end

    test "built-in type falls back to metadata when the column is blank (legacy rows)" do
      doc = StubDoc.new(document_type: "expense_invoice",
                        columns: { vendor_name: nil },
                        metadata: { "vendor_name" => "From Metadata" })
      vendor = ExtractedFieldSet.new(doc).fields.find { |f| f[:key] == "vendor_name" }

      assert_equal "From Metadata", vendor[:value]
    end

    test "custom type with a Hash extraction_schema uses schema fields, edited in metadata" do
      schema = { "policy_no" => { "description" => "Policy number" }, "insurer" => {} }
      doc = StubDoc.new(document_type: "insurance_policy", schema: schema,
                        metadata: { "policy_no" => "AX-200" })
      fields = ExtractedFieldSet.new(doc).fields

      assert_equal %w[policy_no insurer], fields.map { |f| f[:key] }
      assert(fields.all? { |f| f[:store] == :metadata })
      assert_equal "Policy number", fields.first[:label] # schema description wins
      assert_equal "Insurer", fields.last[:label]        # humanised fallback
      assert_equal "AX-200", fields.first[:value]
    end

    test "unknown type without a schema surfaces raw metadata, minus the title" do
      doc = StubDoc.new(document_type: "mystery",
                        metadata: { "title" => "Ignore me", "reference" => "R-1" })
      fields = ExtractedFieldSet.new(doc).fields

      assert_equal %w[reference], fields.map { |f| f[:key] }
      assert_equal "R-1", fields.first[:value]
      assert_equal :metadata, fields.first[:store]
    end

    test "COLUMN_KEYS lists every editable column; ENUM_KEYS flags the enum-backed ones" do
      assert_includes ExtractedFieldSet::COLUMN_KEYS, :vendor_name
      assert_includes ExtractedFieldSet::COLUMN_KEYS, :amount_cents
      # Enums are editable too (as <select>s), so they're in COLUMN_KEYS...
      assert_includes ExtractedFieldSet::COLUMN_KEYS, :expense_category
      assert_includes ExtractedFieldSet::COLUMN_KEYS, :payment_method
      # ...and ENUM_KEYS isolates them for the controller's blank → nil coercion.
      assert_equal %i[expense_category payment_method].sort, ExtractedFieldSet::ENUM_KEYS.sort
    end
  end
end
