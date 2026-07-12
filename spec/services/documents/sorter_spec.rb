# frozen_string_literal: true

require "rails_helper"

RSpec.describe Documents::Sorter do
  let(:workspace) { create(:workspace) }

  # A DocumentType with the expense_invoice builtin schema so we can test
  # extracted-field sort keys.
  let(:expense_invoice_type) do
    create(:document_type, workspace: workspace,
           name: "expense_invoice",
           extraction_schema: DocumentTypes::BuiltinSchemas.for("expense_invoice"))
  end

  def from_params(params, document_type: nil)
    described_class.from_params(params, document_type: document_type)
  end

  # ── Universal keys ────────────────────────────────────────────────────────

  describe '"added" sort key' do
    it "is active with default direction asc" do
      sorter = from_params({ sort: "added" })
      expect(sorter.active?).to be(true)
      expect(sorter.key).to eq("added")
      expect(sorter.dir).to eq("asc")
    end

    it 'uses documents.created_at in the ORDER BY fragment' do
      sorter = from_params({ sort: "added", dir: "desc" })
      sql = sorter.to_h  # internal; verify via apply
      scope = Document.all
      applied = sorter.apply(scope)
      expect(applied.to_sql).to include("documents.created_at")
      expect(applied.to_sql).to include("DESC NULLS LAST")
    end

    it "includes a stable created_at/id tiebreak" do
      sorter = from_params({ sort: "added", dir: "asc" })
      sql = sorter.apply(Document.all).to_sql
      expect(sql).to match(/created_at.*DESC/i)
      expect(sql).to match(/"id" DESC/i)
    end
  end

  describe '"name" sort key' do
    it "is active" do
      sorter = from_params({ sort: "name" })
      expect(sorter.active?).to be(true)
    end

    it "SQL approximates display_title (title -> vendor -> client -> filename)" do
      sorter = from_params({ sort: "name", dir: "asc" })
      sql = sorter.apply(Document.all).to_sql
      expect(sql).to include("metadata->>'title'")
      expect(sql).to include("metadata->>'vendor_name'")
      expect(sql).to include("metadata->>'client_name'")
      expect(sql).to include("canonical_filename")
      expect(sql).to include("NULLS LAST")
    end

    it "respects desc direction" do
      sorter = from_params({ sort: "name", dir: "desc" })
      sql = sorter.apply(Document.all).to_sql
      expect(sql).to include("DESC NULLS LAST")
    end
  end

  # ── Direction validation ──────────────────────────────────────────────────

  describe "direction validation" do
    it 'defaults to asc when dir is absent' do
      sorter = from_params({ sort: "added" })
      expect(sorter.dir).to eq("asc")
    end

    it 'accepts "desc"' do
      sorter = from_params({ sort: "added", dir: "desc" })
      expect(sorter.dir).to eq("desc")
    end

    it 'coerces anything else to asc' do
      sorter = from_params({ sort: "added", dir: "random" })
      expect(sorter.dir).to eq("asc")

      sorter2 = from_params({ sort: "added", dir: "" })
      expect(sorter2.dir).to eq("asc")
    end
  end

  # ── Extracted-field keys ──────────────────────────────────────────────────

  describe "extracted-field sort keys" do
    it "resolves when document_type is given and key exists in schema" do
      sorter = from_params({ sort: "amount_cents", dir: "desc" }, document_type: expense_invoice_type)
      expect(sorter.active?).to be(true)
      expect(sorter.key).to eq("amount_cents")
    end

    it "SQL for money key includes guarded bigint cast and NULLS LAST" do
      sorter = from_params({ sort: "amount_cents", dir: "asc" }, document_type: expense_invoice_type)
      sql = sorter.apply(Document.all).to_sql
      expect(sql).to include("bigint")
      expect(sql).to include("NULLS LAST")
    end

    it "SQL for date key includes regex guard and NULLS LAST" do
      sorter = from_params({ sort: "document_date", dir: "asc" }, document_type: expense_invoice_type)
      sql = sorter.apply(Document.all).to_sql
      expect(sql).to include("NULLS LAST")
      expect(sql).to include("document_date")
    end

    it "is inactive when document_type is nil (multi-type or no type)" do
      sorter = from_params({ sort: "amount_cents" }, document_type: nil)
      expect(sorter.active?).to be(false)
    end

    it "is inactive when key is not in the type's schema" do
      sorter = from_params({ sort: "nonexistent_field" }, document_type: expense_invoice_type)
      expect(sorter.active?).to be(false)
    end
  end

  # ── Unknown / hostile key rejection ──────────────────────────────────────

  describe "unknown and hostile sort keys" do
    it "returns an inactive sorter for a blank sort param" do
      expect(from_params({}).active?).to be(false)
      expect(from_params({ sort: "" }).active?).to be(false)
    end

    it "returns an inactive sorter for a completely unknown key" do
      expect(from_params({ sort: "nonsense" }).active?).to be(false)
    end

    it "rejects a hostile SQL injection attempt" do
      hostile = "; DROP TABLE documents; --"
      sorter = from_params({ sort: hostile })
      expect(sorter.active?).to be(false)
    end

    it "rejects a hostile key even if document_type is given" do
      hostile = "'; DROP TABLE documents; --"
      sorter = from_params({ sort: hostile }, document_type: expense_invoice_type)
      expect(sorter.active?).to be(false)
    end
  end

  # ── to_h round-trip ───────────────────────────────────────────────────────

  describe "#to_h" do
    it "returns { sort, dir } when active" do
      sorter = from_params({ sort: "added", dir: "desc" })
      expect(sorter.to_h).to eq({ "sort" => "added", "dir" => "desc" })
    end

    it "returns empty hash when inactive" do
      sorter = from_params({})
      expect(sorter.to_h).to eq({})
    end

    it "round-trips through from_params" do
      original = from_params({ sort: "added", dir: "desc" })
      rebuilt  = from_params(original.to_h)
      expect(rebuilt.active?).to be(true)
      expect(rebuilt.key).to eq(original.key)
      expect(rebuilt.dir).to eq(original.dir)
    end

    it "includes asc direction even though it's the default" do
      sorter = from_params({ sort: "name", dir: "asc" })
      expect(sorter.to_h).to eq({ "sort" => "name", "dir" => "asc" })
    end
  end

  # ── apply is a no-op when inactive ───────────────────────────────────────

  describe "#apply" do
    it "returns scope unchanged when inactive" do
      sorter = from_params({})
      scope  = Document.all
      expect(sorter.apply(scope).to_sql).to eq(scope.to_sql)
    end
  end
end
