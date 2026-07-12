# frozen_string_literal: true

require "rails_helper"

RSpec.describe DocumentTypes::BuiltinSchemas do
  describe ".for" do
    it "returns a schema hash for each built-in type" do
      %w[expense_invoice revenue_invoice receipt bank_statement other].each do |type|
        expect(described_class.for(type)).to be_a(Hash), "expected #{type} to be present"
      end
    end

    it "returns nil for unknown types" do
      expect(described_class.for("mystery_type")).to be_nil
    end
  end

  describe "ALL" do
    subject(:all) { described_class::ALL }

    it "contains exactly the five built-in types" do
      expect(all.keys).to match_array(%w[expense_invoice revenue_invoice receipt bank_statement other])
    end
  end

  describe "field type mapping" do
    it "maps *_cents fields to 'money'" do
      %w[expense_invoice revenue_invoice receipt bank_statement].each do |type|
        schema = described_class.for(type)
        cents_keys = schema.keys.select { |k| k.end_with?("_cents") }
        expect(cents_keys).not_to be_empty, "#{type} has no cents keys"
        cents_keys.each do |k|
          expect(schema[k]["type"]).to eq("money"), "#{type}.#{k} should be 'money'"
        end
      end
    end

    it "maps date fields to 'date'" do
      date_keys = %w[document_date due_date period_start period_end]
      described_class::ALL.each do |type_name, schema|
        schema.each do |key, defn|
          next unless date_keys.include?(key)

          expect(defn["type"]).to eq("date"), "#{type_name}.#{key} should be 'date'"
        end
      end
    end

    it "maps tax_rate to 'number'" do
      [ "expense_invoice", "revenue_invoice" ].each do |type|
        schema = described_class.for(type)
        expect(schema["tax_rate"]["type"]).to eq("number")
      end
    end

    it "maps expense_category to 'enum' with EXPENSE_CATEGORIES values" do
      field = described_class.for("expense_invoice")["expense_category"]
      expect(field["type"]).to eq("enum")
      expect(field["values"]).to eq(Document::EXPENSE_CATEGORIES)
    end

    it "maps payment_method to 'enum' with PAYMENT_METHODS values" do
      field = described_class.for("receipt")["payment_method"]
      expect(field["type"]).to eq("enum")
      expect(field["values"]).to eq(Document::PAYMENT_METHODS)
    end

    it "maps currency to 'string'" do
      %w[expense_invoice revenue_invoice bank_statement].each do |type|
        schema = described_class.for(type)
        expect(schema["currency"]["type"]).to eq("string"), "#{type}.currency should be 'string'"
      end
    end
  end

  describe "positions" do
    it "are 1-based and sequential within each type" do
      described_class::ALL.each do |type_name, schema|
        positions = schema.values.map { |d| d["position"] }.sort
        expect(positions).to eq((1..schema.size).to_a),
          "#{type_name} positions are not sequential: #{positions.inspect}"
      end
    end
  end

  describe "label_keys" do
    it "all label_keys resolve via I18n in the :en locale" do
      I18n.with_locale(:en) do
        described_class::ALL.each do |type_name, schema|
          schema.each do |key, defn|
            label_key = defn["label_key"]
            expect(label_key).to be_present, "#{type_name}.#{key} missing label_key"
            expect { I18n.t(label_key, raise: true) }.not_to raise_error,
              "#{type_name}.#{key} label_key '#{label_key}' missing from en locale"
          end
        end
      end
    end
  end

  describe "other type" do
    let(:schema) { described_class.for("other") }

    it "has vendor_name with entity_name label_key" do
      field = schema["vendor_name"]
      expect(field["label_key"]).to eq("documents.forms.other.entity_name")
    end

    it "has document_date as a date field" do
      expect(schema["document_date"]["type"]).to eq("date")
    end
  end

  describe "expense_invoice" do
    let(:schema) { described_class.for("expense_invoice") }

    it "includes due_date after document_date" do
      doc_pos  = schema["document_date"]["position"]
      due_pos  = schema["due_date"]["position"]
      expect(due_pos).to be > doc_pos
    end

    it "has expense_category as the last field" do
      last_pos = schema.values.map { |d| d["position"] }.max
      expect(schema["expense_category"]["position"]).to eq(last_pos)
    end
  end

  describe "revenue_invoice" do
    it "includes due_date" do
      expect(described_class.for("revenue_invoice")).to have_key("due_date")
    end
  end
end
