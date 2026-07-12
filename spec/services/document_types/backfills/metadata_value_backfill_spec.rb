# frozen_string_literal: true

require "rails_helper"

RSpec.describe DocumentTypes::Backfills::MetadataValueBackfill do
  subject(:backfill) { described_class.new }

  let(:conn) { ActiveRecord::Base.connection }
  let(:workspace) { create(:workspace) }

  # These specs plant values in the legacy columns, which a later migration
  # drops. On the post-drop schema the backfill is a guarded no-op — assert
  # exactly that and skip the column-era behaviors.
  before do
    unless ActiveRecord::Base.connection.column_exists?(:documents, :vendor_name)
      skip "legacy extracted-value columns are dropped — backfill is a no-op"
    end
  end

  it "is a safe no-op when the legacy columns no longer exist" do
    allow(ActiveRecord::Base.connection).to receive(:column_exists?).and_call_original
    allow(ActiveRecord::Base.connection).to receive(:column_exists?)
      .with(:documents, :vendor_name).and_return(false)

    expect { described_class.run! }.not_to raise_error
  end

  # Reload raw metadata JSONB as a Ruby hash for the given document id.
  def raw_metadata(doc_id)
    row = conn.select_one("SELECT metadata FROM documents WHERE id = '#{doc_id}'")
    val = row&.dig("metadata")
    val ? JSON.parse(val) : nil
  end

  # Write legacy column values directly, bypassing the Document model's
  # extracted_field writers (which would write to metadata instead of columns).
  def set_legacy_columns(doc_id, columns)
    sets = columns.map { |col, _| "#{col} = ?" }.join(", ")
    vals = columns.values
    conn.execute(
      ActiveRecord::Base.sanitize_sql_array(
        [ "UPDATE documents SET #{sets} WHERE id = ?", *vals, doc_id.to_s ]
      )
    )
  end

  describe ".run!" do
    it "delegates to a new instance" do
      expect_any_instance_of(described_class).to receive(:run!)
      described_class.run!
    end
  end

  describe "#run!" do
    context "string column wins over existing metadata value" do
      let!(:doc) { create(:document, workspace: workspace, vendor_name: "Metadata Corp") }

      before do
        # metadata["vendor_name"] = "Metadata Corp" (set by factory via extracted_field)
        # Now put a different value in the legacy column
        set_legacy_columns(doc.id, "vendor_name" => "Column Corp")
      end

      it "overwrites metadata with the column value" do
        backfill.run!
        expect(raw_metadata(doc.id)["vendor_name"]).to eq("Column Corp")
      end
    end

    context "NULL column leaves existing metadata value untouched" do
      let!(:doc) { create(:document, workspace: workspace, vendor_name: "Metadata Only") }

      it "keeps the metadata value when the column is NULL" do
        # Factory sets metadata["vendor_name"] via extracted_field; column is NULL (ignored)
        backfill.run!
        expect(raw_metadata(doc.id)["vendor_name"]).to eq("Metadata Only")
      end
    end

    context "integer expense_category is converted to string key" do
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_columns(doc.id, "expense_category" => 3) } # 3 = utilities

      it "writes the string key to metadata" do
        backfill.run!
        expect(raw_metadata(doc.id)["expense_category"]).to eq("utilities")
      end
    end

    context "expense_category integer maps correctly for all values" do
      expected = {
        0 => "travel", 1 => "meals", 2 => "office_supplies", 3 => "utilities",
        4 => "rent", 5 => "software", 6 => "professional_services",
        7 => "equipment", 8 => "marketing", 9 => "other"
      }

      expected.each do |int_val, name|
        it "maps #{int_val} to '#{name}'" do
          doc = create(:document, workspace: workspace)
          set_legacy_columns(doc.id, "expense_category" => int_val)
          backfill.run!
          expect(raw_metadata(doc.id)["expense_category"]).to eq(name)
        end
      end
    end

    context "date columns are converted to ISO-8601 strings" do
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        set_legacy_columns(doc.id,
          "document_date" => "2024-03-15",
          "due_date"      => "2024-04-01",
          "period_start"  => "2024-01-01",
          "period_end"    => "2024-01-31")
      end

      it "writes YYYY-MM-DD strings to metadata" do
        backfill.run!
        meta = raw_metadata(doc.id)
        expect(meta["document_date"]).to eq("2024-03-15")
        expect(meta["due_date"]).to eq("2024-04-01")
        expect(meta["period_start"]).to eq("2024-01-01")
        expect(meta["period_end"]).to eq("2024-01-31")
      end
    end

    context "NULL date column leaves metadata key untouched" do
      let!(:doc) { create(:document, workspace: workspace, document_date: "2025-06-01") }

      it "keeps metadata document_date when the column is NULL" do
        # column is NULL (ignored); metadata has the value from extracted_field writer
        backfill.run!
        expect(raw_metadata(doc.id)["document_date"]).to eq("2025-06-01")
      end
    end

    context "boolean company_vat_present" do
      it "writes true to metadata when column is TRUE" do
        doc = create(:document, workspace: workspace)
        set_legacy_columns(doc.id, "company_vat_present" => true)
        backfill.run!
        expect(raw_metadata(doc.id)["company_vat_present"]).to eq(true)
      end

      it "writes false to metadata when column is FALSE (not stripped)" do
        doc = create(:document, workspace: workspace)
        set_legacy_columns(doc.id, "company_vat_present" => false)
        backfill.run!
        expect(raw_metadata(doc.id)["company_vat_present"]).to eq(false)
      end

      it "leaves metadata key untouched when column is NULL" do
        doc = create(:document, workspace: workspace, company_vat_present: true)
        # column is NULL (ignored column); metadata has true from extracted_field writer
        backfill.run!
        expect(raw_metadata(doc.id)["company_vat_present"]).to eq(true)
      end
    end

    context "numeric tax_rate" do
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_columns(doc.id, "tax_rate" => 23.0) }

      it "writes a JSON number to metadata" do
        backfill.run!
        expect(raw_metadata(doc.id)["tax_rate"]).to eq(23.0)
      end
    end

    context "integer cents fields" do
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        set_legacy_columns(doc.id,
          "amount_cents"            => 12345,
          "tax_amount_cents"        => 2837,
          "opening_balance_cents"   => 500000,
          "closing_balance_cents"   => 450000)
      end

      it "writes integer values as JSON numbers" do
        backfill.run!
        meta = raw_metadata(doc.id)
        expect(meta["amount_cents"]).to eq(12345)
        expect(meta["tax_amount_cents"]).to eq(2837)
        expect(meta["opening_balance_cents"]).to eq(500000)
        expect(meta["closing_balance_cents"]).to eq(450000)
      end
    end

    context "document with NULL metadata column" do
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        # Force metadata to NULL and set some legacy columns
        conn.execute("UPDATE documents SET metadata = NULL WHERE id = '#{doc.id}'")
        set_legacy_columns(doc.id, "vendor_name" => "From Column", "invoice_number" => "INV-001")
      end

      it "initialises metadata from legacy columns without crashing" do
        expect { backfill.run! }.not_to raise_error
        meta = raw_metadata(doc.id)
        expect(meta["vendor_name"]).to eq("From Column")
        expect(meta["invoice_number"]).to eq("INV-001")
      end
    end

    context "non-legacy metadata keys are preserved" do
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        # Write an arbitrary non-legacy key directly into metadata
        conn.execute(<<~SQL)
          UPDATE documents
          SET metadata = jsonb_build_object('custom_field', 'keep me', 'vendor_name', 'Old Metadata')
          WHERE id = '#{doc.id}'
        SQL
        # And set the vendor_name column to something else
        set_legacy_columns(doc.id, "vendor_name" => "New Column")
      end

      it "preserves non-legacy metadata keys" do
        backfill.run!
        meta = raw_metadata(doc.id)
        expect(meta["custom_field"]).to eq("keep me")
      end

      it "overwrites the overlapping key with the column value" do
        backfill.run!
        meta = raw_metadata(doc.id)
        expect(meta["vendor_name"]).to eq("New Column")
      end
    end

    context "idempotency" do
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        set_legacy_columns(doc.id,
          "vendor_name"    => "Idempotent Corp",
          "amount_cents"   => 9900,
          "document_date"  => "2024-06-01",
          "expense_category" => 2)
      end

      it "produces identical metadata on second run" do
        backfill.run!
        meta_after_first = raw_metadata(doc.id)

        backfill.run!
        meta_after_second = raw_metadata(doc.id)

        expect(meta_after_second).to eq(meta_after_first)
      end
    end
  end
end
