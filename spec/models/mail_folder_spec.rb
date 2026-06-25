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

  describe "icon" do
    it "is valid when blank and falls back to the default glyph" do
      mf = MailFolder.new(workspace: workspace, name: "Receipts", icon: "")
      expect(mf).to be_valid
      expect(mf.display_icon).to eq(Campbooks::Icon::DEFAULT)
    end

    it "accepts a known icon name and exposes it via display_icon" do
      mf = MailFolder.new(workspace: workspace, name: "Receipts", icon: "currency-dollar")
      expect(mf).to be_valid
      expect(mf.display_icon).to eq("currency-dollar")
    end

    it "rejects an unknown icon name" do
      mf = MailFolder.new(workspace: workspace, name: "Receipts", icon: "definitely-not-an-icon")
      expect(mf).not_to be_valid
      expect(mf.errors[:icon]).to be_present
    end
  end

  describe "nesting" do
    it "associates a parent and children and reports depth" do
      parent = MailFolder.create!(workspace: workspace, name: "Work")
      child = MailFolder.create!(workspace: workspace, name: "Clients", parent: parent)
      expect(child.parent).to eq(parent)
      expect(parent.children).to include(child)
      expect(parent.depth).to eq(0)
      expect(child.depth).to eq(1)
    end

    it "rejects a folder as its own parent" do
      folder = MailFolder.create!(workspace: workspace, name: "Work")
      folder.parent = folder
      expect(folder).not_to be_valid
      expect(folder.errors[:parent]).to be_present
    end

    it "rejects moving a folder under one of its own descendants" do
      a = MailFolder.create!(workspace: workspace, name: "A")
      b = MailFolder.create!(workspace: workspace, name: "B", parent: a)
      a.parent = b
      expect(a).not_to be_valid
      expect(a.errors[:parent]).to be_present
    end

    it "rejects a parent in a different workspace" do
      other = MailFolder.create!(workspace: create(:workspace), name: "Other")
      folder = MailFolder.new(workspace: workspace, name: "Work", parent: other)
      expect(folder).not_to be_valid
    end

    it "enforces the maximum nesting depth" do
      a = MailFolder.create!(workspace: workspace, name: "A")
      b = MailFolder.create!(workspace: workspace, name: "B", parent: a)
      c = MailFolder.create!(workspace: workspace, name: "C", parent: b)
      too_deep = MailFolder.new(workspace: workspace, name: "D", parent: c)
      expect(too_deep).not_to be_valid
      expect(too_deep.errors[:parent]).to be_present
    end

    it "orphans children to top level when their parent is destroyed" do
      parent = MailFolder.create!(workspace: workspace, name: "Work")
      child = MailFolder.create!(workspace: workspace, name: "Clients", parent: parent)
      parent.destroy
      expect(child.reload.parent_id).to be_nil
    end

    it "lists root folders only" do
      root = MailFolder.create!(workspace: workspace, name: "Root")
      MailFolder.create!(workspace: workspace, name: "Child", parent: root)
      expect(MailFolder.where(workspace: workspace).roots).to contain_exactly(root)
    end
  end

  describe ".next_position_for" do
    it "returns 0 for an empty workspace and the next slot otherwise" do
      expect(described_class.next_position_for(workspace)).to eq(0)
      MailFolder.create!(workspace: workspace, name: "A", position: 0)
      MailFolder.create!(workspace: workspace, name: "B", position: 1)
      expect(described_class.next_position_for(workspace)).to eq(2)
    end
  end

  describe ".document_counts" do
    it "maps folder id → filed-document count, omitting empty folders" do
      folder = MailFolder.create!(workspace: workspace, name: "Receipts")
      empty = MailFolder.create!(workspace: workspace, name: "Empty")
      folder.documents << create(:document, workspace: workspace)
      folder.documents << create(:document, workspace: workspace)

      counts = described_class.document_counts([ folder, empty ])

      expect(counts[folder.id]).to eq(2)
      expect(counts[empty.id]).to be_nil
    end

    it "returns an empty hash when given no folders" do
      expect(described_class.document_counts([])).to eq({})
    end
  end
end
