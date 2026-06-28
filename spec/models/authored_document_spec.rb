require "rails_helper"

RSpec.describe AuthoredDocument, type: :model do
  describe "validations" do
    it "is valid with title and workspace" do
      doc = build(:authored_document, title: "My Document")
      expect(doc).to be_valid
    end

    it "is invalid without title" do
      doc = build(:authored_document, title: "")
      expect(doc).not_to be_valid
      expect(doc.errors[:title]).to include("can't be blank")
    end

    it "is invalid with title exceeding 255 characters" do
      doc = build(:authored_document, title: "a" * 256)
      expect(doc).not_to be_valid
      expect(doc.errors[:title]).to include("is too long (maximum is 255 characters)")
    end

    it "is valid without html_content" do
      doc = build(:authored_document, title: "My Document", html_content: nil)
      expect(doc).to be_valid
    end

    it "is invalid with html_content exceeding 500,000 characters" do
      doc = build(:authored_document, title: "My Document", html_content: "a" * 500_001)
      expect(doc).not_to be_valid
      expect(doc.errors[:html_content]).to include("is too long (maximum is 500000 characters)")
    end
  end

  describe "associations" do
    it "belongs to workspace" do
      doc = create(:authored_document)
      expect(doc.workspace).to be_present
    end

    it "author is optional" do
      doc = create(:authored_document, author: nil)
      expect(doc.author).to be_nil
    end
  end

  describe "scopes" do
    it ".recent orders by created_at descending" do
      older = create(:authored_document, created_at: 1.day.ago)
      newer = create(:authored_document, created_at: 1.hour.ago)
      expect(AuthoredDocument.recent.to_a).to eq([ newer, older ])
    end
  end

  describe "factory" do
    it "creates a valid record" do
      doc = create(:authored_document)
      expect(doc).to be_persisted
      expect(doc.title).to be_present
    end
  end
end
