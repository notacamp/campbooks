require "rails_helper"

RSpec.describe Documents::ClassificationMemory do
  let(:workspace) { create(:workspace) }
  let!(:invoice_type) { DocumentType.create!(workspace: workspace, name: "expense_invoice", color: "#000", prompt: "t") }
  let!(:receipt_type) { DocumentType.create!(workspace: workspace, name: "receipt", color: "#111", prompt: "t") }

  describe "#suggestion by sender" do
    it "returns the dominant approved type from the same sender (case-insensitive)" do
      create_list(:document, 3, :approved, workspace: workspace, sender_name: "EDP", document_type_id: invoice_type.id)
      target = build(:document, workspace: workspace, sender_name: "edp")

      suggestion = described_class.new(target).suggestion

      expect(suggestion.type_name).to eq("expense_invoice")
      expect(suggestion.source).to eq(:sender)
      expect(suggestion.count).to eq(3)
    end

    it "returns nil when no single type holds a strong majority" do
      create_list(:document, 2, :approved, workspace: workspace, sender_name: "EDP", document_type_id: invoice_type.id)
      create_list(:document, 2, :approved, workspace: workspace, sender_name: "EDP", document_type_id: receipt_type.id)
      target = build(:document, workspace: workspace, sender_name: "EDP")

      expect(described_class.new(target).suggestion).to be_nil
    end

    it "ignores documents that are not yet approved" do
      create_list(:document, 3, :in_review, workspace: workspace, sender_name: "EDP", document_type_id: invoice_type.id)
      target = build(:document, workspace: workspace, sender_name: "EDP")

      expect(described_class.new(target).suggestion).to be_nil
    end
  end

  describe "#suggestion by filename" do
    it "matches approved docs whose filename normalizes to the same stem" do
      3.times do |i|
        doc = build(:document, :approved, workspace: workspace, sender_name: nil, document_type: :receipt)
        doc.original_file.attach(io: StringIO.new("x"), filename: "Recibo_EDP_2026_0#{i}.pdf", content_type: "application/pdf")
        doc.save!
      end
      target = build(:document, workspace: workspace, sender_name: nil)
      target.original_file.attach(io: StringIO.new("y"), filename: "recibo-edp-2026-99.pdf", content_type: "application/pdf")

      suggestion = described_class.new(target).suggestion

      expect(suggestion.type_name).to eq("receipt")
      expect(suggestion.source).to eq(:filename)
    end
  end

  describe "#prompt_hint" do
    it "renders a one-line hint when there is a suggestion" do
      create_list(:document, 3, :approved, workspace: workspace, sender_name: "EDP", document_type_id: invoice_type.id)
      target = build(:document, workspace: workspace, sender_name: "EDP")

      expect(described_class.new(target).prompt_hint).to include("expense_invoice").and include("3 of 3")
    end

    it "is nil with an empty corpus" do
      target = build(:document, workspace: workspace, sender_name: "Nobody")
      expect(described_class.new(target).prompt_hint).to be_nil
    end
  end
end
