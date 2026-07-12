# frozen_string_literal: true

require "rails_helper"

RSpec.describe DocumentTypes::Backfills::ClassificationLinkBackfill do
  subject(:backfill) { described_class.new }

  let(:conn) { ActiveRecord::Base.connection }
  let(:workspace) { create(:workspace) }

  # Creates a DocumentType row with the given name in the given workspace.
  def create_document_type(name:, ws: workspace)
    create(:document_type, name: name, workspace: ws)
  end

  # Returns the current document_type_id for the given document id.
  def doc_type_id_for(doc_id)
    conn.select_one(
      "SELECT document_type_id FROM documents WHERE id = '#{doc_id}'"
    )&.dig("document_type_id")
  end

  # Sets the legacy integer document_type and clears document_type_id via raw SQL
  # so we bypass the app model's ignored_columns and before_save hooks.
  def set_legacy_type(doc_id, int_value, type_id = nil)
    conn.execute(<<~SQL)
      UPDATE documents
      SET document_type = #{int_value.to_i},
          document_type_id = #{type_id ? "'#{type_id}'" : "NULL"}
      WHERE id = '#{doc_id}'
    SQL
  end

  describe ".run!" do
    it "delegates to a new instance" do
      expect_any_instance_of(described_class).to receive(:run!)
      described_class.run!
    end
  end

  describe "#run!" do
    context "when a document has a matching DocumentType in the same workspace" do
      let!(:dt) { create_document_type(name: "expense_invoice") }
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_type(doc.id, 0) } # 0 = expense_invoice

      it "sets document_type_id to the matching DocumentType" do
        backfill.run!
        expect(doc_type_id_for(doc.id)).to eq(dt.id)
      end
    end

    context "when the matching DocumentType is in a different workspace" do
      let(:other_workspace) { create(:workspace) }
      let!(:dt_other) { create_document_type(name: "expense_invoice", ws: other_workspace) }
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_type(doc.id, 0) }

      it "does not set document_type_id (cross-workspace mismatch)" do
        backfill.run!
        expect(doc_type_id_for(doc.id)).to be_nil
      end
    end

    context "when a document already has document_type_id set" do
      let!(:dt) { create_document_type(name: "expense_invoice") }
      let!(:other_dt) { create_document_type(name: "receipt") }
      let!(:doc) { create(:document, workspace: workspace) }

      before do
        # Give doc an existing document_type_id (pointing at dt)
        # and a legacy type of 3 (receipt) — the backfill must not overwrite
        conn.execute(<<~SQL)
          UPDATE documents
          SET document_type = 3,
              document_type_id = '#{dt.id}'
          WHERE id = '#{doc.id}'
        SQL
      end

      it "leaves the existing document_type_id unchanged" do
        backfill.run!
        expect(doc_type_id_for(doc.id)).to eq(dt.id)
      end
    end

    context "when the legacy enum value has no matching document_types row" do
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_type(doc.id, 13) } # 13 = bank_journal_entry (no seed row)

      it "leaves document_type_id NULL when no matching type exists" do
        backfill.run!
        expect(doc_type_id_for(doc.id)).to be_nil
      end
    end

    context "with all 5 common enum values" do
      let(:type_names) do
        { 0 => "expense_invoice", 1 => "revenue_invoice", 2 => "bank_statement",
          3 => "receipt", 4 => "other" }
      end
      let!(:doc_types) { type_names.transform_values { |n| create_document_type(name: n) } }
      let!(:docs) do
        type_names.keys.map do |int_val|
          doc = create(:document, workspace: workspace)
          set_legacy_type(doc.id, int_val)
          [ int_val, doc.id ]
        end.to_h
      end

      it "links each document to its matching type" do
        backfill.run!
        type_names.each do |int_val, name|
          expected_id = doc_types[int_val].id
          actual_id = doc_type_id_for(docs[int_val])
          expect(actual_id).to eq(expected_id), "Expected doc with type #{int_val}(#{name}) to get id #{expected_id}, got #{actual_id}"
        end
      end
    end

    context "idempotency" do
      let!(:dt) { create_document_type(name: "receipt") }
      let!(:doc) { create(:document, workspace: workspace) }

      before { set_legacy_type(doc.id, 3) } # 3 = receipt

      it "produces the same result on a second run" do
        backfill.run!
        id_after_first = doc_type_id_for(doc.id)

        backfill.run!
        id_after_second = doc_type_id_for(doc.id)

        expect(id_after_second).to eq(id_after_first)
        expect(id_after_second).to eq(dt.id)
      end
    end
  end
end
