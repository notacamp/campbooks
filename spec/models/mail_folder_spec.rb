require "rails_helper"

RSpec.describe MailFolder, type: :model do
  let(:workspace) { create(:workspace) }

  it "is valid with a name and workspace" do
    expect(MailFolder.new(workspace: workspace, name: "Receipts")).to be_valid
  end

  it "requires a non-blank name" do
    expect(MailFolder.new(workspace: workspace, name: " ")).not_to be_valid
  end

  it "rejects reserved system folder names (case-insensitive)" do
    %w[Inbox sent ARCHIVE Trash Drafts Spam].each do |reserved|
      mf = MailFolder.new(workspace: workspace, name: reserved)
      expect(mf).not_to be_valid, "expected #{reserved.inspect} to be rejected"
      expect(mf.errors[:name]).to be_present
    end
  end

  it "enforces case-insensitive uniqueness per workspace" do
    MailFolder.create!(workspace: workspace, name: "Receipts")
    expect(MailFolder.new(workspace: workspace, name: "receipts")).not_to be_valid
  end

  it "allows the same name in a different workspace" do
    MailFolder.create!(workspace: workspace, name: "Receipts")
    expect(MailFolder.new(workspace: create(:workspace), name: "Receipts")).to be_valid
  end

  it "normalizes surrounding whitespace in the name" do
    expect(MailFolder.create!(workspace: workspace, name: "  Receipts  ").name).to eq("Receipts")
  end

  describe ".next_position_for" do
    it "returns 0 for an empty workspace and the next slot otherwise" do
      expect(described_class.next_position_for(workspace)).to eq(0)
      MailFolder.create!(workspace: workspace, name: "A", position: 0)
      MailFolder.create!(workspace: workspace, name: "B", position: 1)
      expect(described_class.next_position_for(workspace)).to eq(2)
    end
  end
end
