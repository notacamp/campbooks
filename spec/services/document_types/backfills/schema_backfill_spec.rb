# frozen_string_literal: true

require "rails_helper"

RSpec.describe DocumentTypes::Backfills::SchemaBackfill do
  subject(:backfill) { described_class.new }

  let(:conn) { ActiveRecord::Base.connection }
  let(:workspace) { create(:workspace) }

  # Helper: create a DocumentType row with the given extraction_schema via raw SQL
  # so we bypass any app-level callbacks and control the exact stored value.
  def create_doc_type(name:, schema:)
    conn.execute(
      ActiveRecord::Base.sanitize_sql_array([
        <<~SQL,
          INSERT INTO document_types (id, workspace_id, name, color, extraction_schema, created_at, updated_at)
          VALUES (gen_random_uuid(), ?, ?, '#aabbcc', ?::jsonb, NOW(), NOW())
          RETURNING id
        SQL
        workspace.id.to_s, name, schema.to_json
      ])
    ).first["id"]
  end

  def fetch_schema(id)
    row = conn.select_one("SELECT extraction_schema FROM document_types WHERE id = '#{id}'")
    row ? row["extraction_schema"].then { |v| v ? JSON.parse(v) : nil } : nil
  end

  describe ".run!" do
    it "delegates to a new instance" do
      expect_any_instance_of(described_class).to receive(:run!)
      described_class.run!
    end
  end

  describe "#run!" do
    context "when a built-in type has an empty extraction_schema" do
      let!(:dt_id) { create_doc_type(name: "expense_invoice", schema: nil) }

      it "sets the full canonical schema" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema).to be_a(Hash)
        expect(schema).to have_key("vendor_name")
        expect(schema).to have_key("expense_category")
        expect(schema["amount_cents"]["type"]).to eq("money")
      end
    end

    context "when a built-in type has an existing schema with only some fields" do
      let(:old_schema) do
        {
          "vendor_name" => { "type" => "string", "description" => "Old desc" },
          "amount_cents" => { "type" => "integer" }
        }
      end
      let!(:dt_id) { create_doc_type(name: "expense_invoice", schema: old_schema) }

      it "canonical entries win for overlapping keys" do
        backfill.run!
        schema = fetch_schema(dt_id)
        # Canonical amount_cents should be "money", not the stale "integer"
        expect(schema["amount_cents"]["type"]).to eq("money")
        expect(schema["amount_cents"]["position"]).to be_present
      end

      it "canonical entries win for vendor_name" do
        backfill.run!
        schema = fetch_schema(dt_id)
        # Canonical vendor_name has label_key; the old entry had none
        expect(schema["vendor_name"]["label_key"]).to be_present
      end

      it "adds canonical-only fields not in the old schema" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema).to have_key("expense_category")
        expect(schema).to have_key("due_date")
      end
    end

    context "when a built-in type has user-added extra keys" do
      let(:existing_schema) do
        canonical = DocumentTypes::BuiltinSchemas.for("receipt")
        # Add a user-defined extra key
        canonical.merge("po_number" => { "type" => "string", "description" => "Purchase order" })
      end
      let!(:dt_id) { create_doc_type(name: "receipt", schema: existing_schema) }

      it "preserves the user-added key" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema).to have_key("po_number")
        expect(schema["po_number"]["description"]).to eq("Purchase order")
      end

      it "assigns the user-added key a position after canonical entries" do
        backfill.run!
        schema = fetch_schema(dt_id)
        canonical_positions = DocumentTypes::BuiltinSchemas.for("receipt").values.map { |v| v["position"] }
        max_canonical = canonical_positions.max
        expect(schema["po_number"]["position"]).to be > max_canonical
      end

      it "canonical keys still win (not replaced by user definition)" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema["amount_cents"]["type"]).to eq("money")
      end
    end

    context "when a user-added extra key already has a position" do
      let(:existing_schema) do
        canonical = DocumentTypes::BuiltinSchemas.for("bank_statement")
        canonical.merge("swift_code" => { "type" => "string", "description" => "SWIFT", "position" => 99 })
      end
      let!(:dt_id) { create_doc_type(name: "bank_statement", schema: existing_schema) }

      it "preserves the existing position on the user key" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema["swift_code"]["position"]).to eq(99)
      end
    end

    context "when a custom (non-builtin) document type exists" do
      let!(:dt_id) { create_doc_type(name: "my_custom_type", schema: { "foo" => { "type" => "string" } }) }

      it "leaves custom types untouched" do
        backfill.run!
        schema = fetch_schema(dt_id)
        expect(schema).to eq("foo" => { "type" => "string" })
      end
    end

    context "idempotency" do
      let!(:dt_id) { create_doc_type(name: "revenue_invoice", schema: nil) }

      it "produces identical rows on second run" do
        backfill.run!
        schema_after_first = fetch_schema(dt_id)

        backfill.run!
        schema_after_second = fetch_schema(dt_id)

        expect(schema_after_second).to eq(schema_after_first)
      end
    end

    context "multiple workspaces" do
      let(:workspace2) { create(:workspace) }

      it "processes built-in types in all workspaces" do
        id1 = create_doc_type(name: "expense_invoice", schema: nil)
        id2 = conn.execute(
          ActiveRecord::Base.sanitize_sql_array([
            "INSERT INTO document_types (id, workspace_id, name, color, extraction_schema, created_at, updated_at) " \
            "VALUES (gen_random_uuid(), ?, 'expense_invoice', '#aabbcc', NULL, NOW(), NOW()) RETURNING id",
            workspace2.id.to_s
          ])
        ).first["id"]

        backfill.run!

        expect(fetch_schema(id1)).to have_key("vendor_name")
        expect(fetch_schema(id2)).to have_key("vendor_name")
      end
    end
  end
end
