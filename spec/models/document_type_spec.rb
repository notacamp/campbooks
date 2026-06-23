require 'rails_helper'

RSpec.describe DocumentType, type: :model do
  let(:workspace) { create(:workspace) }

  describe ".default_category_for" do
    it "maps built-in type names to their category" do
      expect(DocumentType.default_category_for("expense_invoice")).to eq("accounting")
      expect(DocumentType.default_category_for("contract")).to eq("legal")
      expect(DocumentType.default_category_for("insurance_policy")).to eq("insurance")
      expect(DocumentType.default_category_for("vehicle_document")).to eq("vehicles")
    end

    it "maps the setup presets' names (including spaced/cased variants)" do
      expect(DocumentType.default_category_for("invoice")).to eq("accounting")
      expect(DocumentType.default_category_for("Bank Statement")).to eq("accounting")
      expect(DocumentType.default_category_for("payslip")).to eq("accounting")
    end

    it "falls back to 'other' for unknown or blank names" do
      expect(DocumentType.default_category_for("my custom type")).to eq("other")
      expect(DocumentType.default_category_for(nil)).to eq("other")
      expect(DocumentType.default_category_for("")).to eq("other")
    end
  end

  describe "assigning a default category on create" do
    it "categorises a recognised type that was created without a category" do
      type = workspace.document_types.create!(name: "expense_invoice", color: "#000")
      expect(type.category).to eq("accounting")
    end

    it "files an unrecognised custom type under 'other' rather than leaving it blank" do
      type = workspace.document_types.create!(name: "Weird Custom Thing", color: "#000")
      expect(type.category).to eq("other")
    end

    it "never overrides an explicitly chosen category" do
      type = workspace.document_types.create!(name: "expense_invoice", color: "#000", category: "legal")
      expect(type.category).to eq("legal")
    end

    it "lets an existing type's category be cleared via update (callback is create-only)" do
      type = workspace.document_types.create!(name: "receipt", color: "#000")
      type.update!(category: nil)
      expect(type.reload.category).to be_nil
    end
  end
end
