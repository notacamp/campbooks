# frozen_string_literal: true

require "rails_helper"

RSpec.describe DocumentTypes::Schema do
  let(:enriched_schema) do
    {
      "vendor_name"  => { "type" => "string", "label_key" => "documents.forms.expense_invoice.vendor_name",
                          "description" => "Vendor Name", "position" => 1 },
      "amount_cents" => { "type" => "money",  "label_key" => "documents.forms.expense_invoice.amount_cents",
                          "description" => "Amount (cents)", "position" => 2 },
      "document_date" => { "type" => "date",   "description" => "Document Date", "position" => 3 },
      "tax_rate"     => { "type" => "number",  "description" => "Tax Rate",       "position" => 4 },
      "status"       => { "type" => "enum",    "description" => "Status",
                          "values" => %w[open closed], "position" => 5 },
      "active"       => { "type" => "boolean", "description" => "Active",         "position" => 6 }
    }
  end

  subject(:schema) { described_class.new(enriched_schema) }

  # ── .for constructor ────────────────────────────────────────────────────────

  describe ".for" do
    it "returns empty schema when classification is nil" do
      s = described_class.for(nil)
      expect(s.any?).to be false
    end

    it "reads extraction_schema from a classification object" do
      cls = double(extraction_schema: { "note" => { "type" => "string", "description" => "Note" } })
      s = described_class.for(cls)
      expect(s.any?).to be true
      expect(s.field("note").type).to eq(:string)
    end
  end

  # ── #fields ─────────────────────────────────────────────────────────────────

  describe "#fields" do
    it "returns fields ordered by position" do
      expect(schema.fields.map(&:key)).to eq(%w[vendor_name amount_cents document_date tax_rate status active])
    end

    it "handles old-format entries (no label_key, no position)" do
      old = described_class.new({
        "policy_no" => { "type" => "string", "description" => "Policy number" },
        "insurer"   => {}
      })
      fields = old.fields
      expect(fields.map(&:key)).to eq(%w[policy_no insurer])
      expect(fields.first.label).to eq("Policy number")  # description wins
      expect(fields.last.label).to eq("Insurer")         # humanized fallback
    end

    it "defaults unknown type to :string" do
      s = described_class.new({ "foo" => { "type" => "mystery", "description" => "X" } })
      expect(s.fields.first.type).to eq(:string)
    end

    it "defaults nil/missing type to :string" do
      s = described_class.new({ "foo" => { "description" => "X" } })
      expect(s.fields.first.type).to eq(:string)
    end

    it "uses insertion order when position is absent" do
      s = described_class.new({
        "c" => { "description" => "C" },
        "a" => { "description" => "A" },
        "b" => { "description" => "B" }
      })
      expect(s.fields.map(&:key)).to eq(%w[c a b])
    end

    it "returns an empty array for nil schema" do
      expect(described_class.new(nil).fields).to eq([])
    end

    it "returns an empty array for non-Hash schema" do
      expect(described_class.new("string schema").fields).to eq([])
    end
  end

  # ── Label precedence ─────────────────────────────────────────────────────────

  describe "label precedence" do
    it "uses I18n translation when label_key is present and resolves" do
      field = schema.field("vendor_name")
      expect(field.label).to eq(I18n.t("documents.forms.expense_invoice.vendor_name", locale: :en))
    end

    it "falls back to description when label_key is absent" do
      field = schema.field("document_date")
      expect(field.label).to eq("Document Date")
    end

    it "falls back to humanized key when neither label_key nor description" do
      s = described_class.new({ "my_custom_field" => {} })
      expect(s.field("my_custom_field").label).to eq("My custom field")
    end

    it "falls back to description when label_key is present but missing from locale" do
      s = described_class.new({ "x" => { "label_key" => "nonexistent.key.abc", "description" => "Fallback" } })
      expect(s.field("x").label).to eq("Fallback")
    end
  end

  # ── #any? and #field ─────────────────────────────────────────────────────────

  describe "#any?" do
    it "returns true when schema has fields" do
      expect(schema.any?).to be true
    end

    it "returns false for empty schema" do
      expect(described_class.new({}).any?).to be false
    end
  end

  describe "#field" do
    it "returns the Field for an existing key" do
      expect(schema.field("vendor_name")).to be_a(described_class::Field)
    end

    it "returns nil for an absent key" do
      expect(schema.field("nonexistent")).to be_nil
    end
  end

  # ── Field#kind ───────────────────────────────────────────────────────────────

  describe "Field#kind" do
    it "returns :money for money type" do
      expect(schema.field("amount_cents").kind).to eq(:money)
    end

    it "returns :date for date type" do
      expect(schema.field("document_date").kind).to eq(:date)
    end

    it "returns :enum for enum type" do
      expect(schema.field("status").kind).to eq(:enum)
    end

    it "returns :text for string type" do
      expect(schema.field("vendor_name").kind).to eq(:text)
    end

    it "returns :text for number type" do
      expect(schema.field("tax_rate").kind).to eq(:text)
    end

    it "returns :boolean for boolean type" do
      expect(schema.field("active").kind).to eq(:boolean)
    end
  end

  # ── Field#coerce ─────────────────────────────────────────────────────────────

  describe "Field#coerce" do
    let(:string_field) { schema.field("vendor_name") }
    let(:money_field)  { schema.field("amount_cents") }
    let(:date_field)   { schema.field("document_date") }
    let(:number_field) { schema.field("tax_rate") }
    let(:enum_field)   { schema.field("status") }
    let(:bool_field)   { schema.field("active") }

    # nil / blank
    it "returns nil for nil" do
      [ string_field, money_field, date_field, number_field, enum_field, bool_field ].each do |f|
        expect(f.coerce(nil)).to be_nil, "expected nil for #{f.key}"
      end
    end

    it "returns nil for blank string" do
      [ string_field, money_field ].each do |f|
        expect(f.coerce("  ")).to be_nil
      end
    end

    # string
    it "strips whitespace for string" do
      expect(string_field.coerce("  Acme Lda  ")).to eq("Acme Lda")
    end

    # money/integer
    it "converts integer string to Integer for money" do
      expect(money_field.coerce("12345")).to eq(12_345)
    end

    it "converts '12345.0' to 12345 for money" do
      expect(money_field.coerce("12345.0")).to eq(12_345)
    end

    it "converts Integer to Integer for money" do
      expect(money_field.coerce(12_345)).to eq(12_345)
    end

    it "converts Float to Integer for money" do
      expect(money_field.coerce(12_345.9)).to eq(12_345)
    end

    it "returns nil for garbage money input" do
      expect(money_field.coerce("garbage")).to be_nil
    end

    # number
    it "converts numeric string to Float for number" do
      expect(number_field.coerce("23.5")).to eq(23.5)
    end

    it "converts Integer to Float for number" do
      expect(number_field.coerce(23)).to eq(23.0)
    end

    it "returns nil for garbage number input" do
      expect(number_field.coerce("abc")).to be_nil
    end

    # date
    it "converts ISO-8601 string to ISO-8601 string for date" do
      expect(date_field.coerce("2024-03-15")).to eq("2024-03-15")
    end

    it "converts Date object to ISO-8601 string for date" do
      expect(date_field.coerce(Date.new(2024, 3, 15))).to eq("2024-03-15")
    end

    it "converts parseable string to ISO-8601 string for date" do
      expect(date_field.coerce("15/03/2024")).to eq("2024-03-15")
    end

    it "returns nil for garbage date input" do
      expect(date_field.coerce("not-a-date")).to be_nil
    end

    # enum
    it "accepts valid enum value" do
      expect(enum_field.coerce("open")).to eq("open")
    end

    it "returns nil for value not in enum_values" do
      expect(enum_field.coerce("pending")).to be_nil
    end

    # boolean
    it "coerces true/false/strings/numbers for boolean" do
      expect(bool_field.coerce(true)).to   eq(true)
      expect(bool_field.coerce("true")).to  eq(true)
      expect(bool_field.coerce("1")).to     eq(true)
      expect(bool_field.coerce(false)).to eq(false)
      expect(bool_field.coerce("false")).to eq(false)
      expect(bool_field.coerce("0")).to     eq(false)
    end

    it "returns nil for unrecognized boolean input" do
      expect(bool_field.coerce("yes")).to be_nil
    end
  end

  # ── Field#read ───────────────────────────────────────────────────────────────

  describe "Field#read" do
    let(:meta) do
      {
        "vendor_name"   => "Acme Lda",
        "amount_cents"  => 12_345,
        "document_date" => "2024-03-15",
        "tax_rate"      => "23.5",
        "status"        => "open",
        "active"        => "true"
      }
    end

    it "reads string as raw string" do
      expect(schema.field("vendor_name").read(meta)).to eq("Acme Lda")
    end

    it "reads money/integer as Integer" do
      expect(schema.field("amount_cents").read(meta)).to eq(12_345)
    end

    it "reads date as Date object" do
      expect(schema.field("document_date").read(meta)).to eq(Date.new(2024, 3, 15))
    end

    it "reads number as Float" do
      expect(schema.field("tax_rate").read(meta)).to eq(23.5)
    end

    it "reads boolean 'true' as true" do
      expect(schema.field("active").read(meta)).to eq(true)
    end

    it "returns nil for absent key" do
      expect(schema.field("vendor_name").read({})).to be_nil
    end

    it "is nil-safe when metadata is nil" do
      expect(schema.field("vendor_name").read(nil)).to be_nil
    end

    it "returns nil for invalid date stored in metadata" do
      expect(schema.field("document_date").read("document_date" => "not-a-date")).to be_nil
    end
  end

  # ── Field#order_sql ──────────────────────────────────────────────────────────

  describe "Field#order_sql" do
    it "generates LOWER/ILIKE-style for string fields" do
      sql = schema.field("vendor_name").order_sql(:asc)
      expect(sql).to match(/LOWER\(documents\.metadata->>'vendor_name'\) ASC NULLS LAST/i)
    end

    it "generates LOWER DESC for string fields" do
      sql = schema.field("vendor_name").order_sql(:desc)
      expect(sql).to include("DESC NULLS LAST")
    end

    it "generates CASE/::bigint for money fields" do
      sql = schema.field("amount_cents").order_sql(:asc)
      expect(sql).to include("::bigint")
      expect(sql).to include("ASC NULLS LAST")
      expect(sql).to match(/CASE WHEN documents\.metadata->>'amount_cents'/)
    end

    it "generates CASE/::numeric for number fields" do
      sql = schema.field("tax_rate").order_sql(:asc)
      expect(sql).to include("::numeric")
    end

    it "generates no cast for date fields (lexicographic sort)" do
      sql = schema.field("document_date").order_sql(:asc)
      expect(sql).not_to include("::")
      expect(sql).to match(/CASE WHEN documents\.metadata->>'document_date'/)
    end

    it "generates boolean equality sort" do
      sql = schema.field("active").order_sql(:asc)
      expect(sql).to include("= 'true'")
      expect(sql).to include("ASC NULLS LAST")
    end

    it "safely quotes a hostile injection key" do
      hostile = "foo'); DROP TABLE documents;--"
      s = described_class.new({ hostile => { "type" => "string", "description" => "X" } })
      field = s.field(hostile)
      sql = field.order_sql(:asc)
      quoted = ActiveRecord::Base.connection.quote(hostile)
      expect(sql).to include(quoted)
      # The dangerous payload is inside a SQL string literal — no bare semicolon
      expect(sql).to match(/LOWER\(documents\.metadata->>#{Regexp.escape(quoted)}\)/)
    end

    it "defaults to ASC for unrecognized direction" do
      sql = schema.field("vendor_name").order_sql(:sideways)
      expect(sql).to include("ASC NULLS LAST")
    end
  end

  # ── Field#apply_predicate ────────────────────────────────────────────────────

  describe "Field#apply_predicate" do
    let(:base_scope) { Document.all }
    let(:vendor_field) { schema.field("vendor_name") }
    let(:amount_field) { schema.field("amount_cents") }
    let(:date_field)   { schema.field("document_date") }
    let(:status_field) { schema.field("status") }
    let(:active_field) { schema.field("active") }
    let(:number_field) { schema.field("tax_rate") }

    it "applies ILIKE for :contains on string fields" do
      sql = vendor_field.apply_predicate(base_scope, :contains, "Acme").to_sql
      expect(sql).to match(/ILIKE/i)
      expect(sql).to include("%Acme%")
    end

    it "escapes LIKE wildcards in :contains" do
      sql = vendor_field.apply_predicate(base_scope, :contains, "50% off").to_sql
      expect(sql).to include("50\\% off")
    end

    it "applies equality for :eq on string fields" do
      sql = vendor_field.apply_predicate(base_scope, :eq, "Acme").to_sql
      expect(sql).to match(/metadata->>'vendor_name' = 'Acme'/i)
    end

    it "returns scope unchanged for blank value on :contains" do
      result = vendor_field.apply_predicate(base_scope, :contains, "")
      expect(result.to_sql).to eq(base_scope.to_sql)
    end

    it "applies :min for integer/money fields" do
      sql = amount_field.apply_predicate(base_scope, :min, 10_000).to_sql
      expect(sql).to include(">=")
      expect(sql).to include("bigint")
    end

    it "applies :max for integer/money fields" do
      sql = amount_field.apply_predicate(base_scope, :max, 50_000).to_sql
      expect(sql).to include("<=")
    end

    it "returns scope unchanged for garbage value on :min" do
      result = amount_field.apply_predicate(base_scope, :min, "garbage")
      expect(result.to_sql).to eq(base_scope.to_sql)
    end

    it "applies :min for number fields" do
      sql = number_field.apply_predicate(base_scope, :min, 10).to_sql
      expect(sql).to include(">=")
      expect(sql).to include("numeric")
    end

    it "applies :from for date fields" do
      sql = date_field.apply_predicate(base_scope, :from, "2024-01-01").to_sql
      expect(sql).to include(">=")
      expect(sql).to include("2024-01-01")
    end

    it "applies :to for date fields" do
      sql = date_field.apply_predicate(base_scope, :to, "2024-12-31").to_sql
      expect(sql).to include("<=")
      expect(sql).to include("2024-12-31")
    end

    it "returns scope unchanged for garbage date on :from" do
      result = date_field.apply_predicate(base_scope, :from, "not-a-date")
      expect(result.to_sql).to eq(base_scope.to_sql)
    end

    it "applies :in for enum fields" do
      sql = status_field.apply_predicate(base_scope, :in, %w[open closed]).to_sql
      expect(sql).to include("IN")
    end

    it "returns scope unchanged for empty array on :in" do
      result = status_field.apply_predicate(base_scope, :in, [])
      expect(result.to_sql).to eq(base_scope.to_sql)
    end

    it "applies :eq for enum fields" do
      sql = status_field.apply_predicate(base_scope, :eq, "open").to_sql
      expect(sql).to include("= 'open'")
    end

    it "applies :eq for boolean fields" do
      sql = active_field.apply_predicate(base_scope, :eq, true).to_sql
      expect(sql).to include("= 'true'")
    end

    it "returns scope unchanged for unknown op" do
      result = vendor_field.apply_predicate(base_scope, :unknown_op, "Acme")
      expect(result.to_sql).to eq(base_scope.to_sql)
    end
  end
end
