require "rails_helper"

RSpec.describe EmailFolder, type: :model do
  describe ".droppable_name?" do
    it "rejects the outbound/compose folders (Sent, Drafts)" do
      expect(EmailFolder.droppable_name?("Sent")).to be(false)
      expect(EmailFolder.droppable_name?("Drafts")).to be(false)
    end

    it "allows every other folder" do
      %w[Inbox Archive Spam Trash Receipts].each do |name|
        expect(EmailFolder.droppable_name?(name)).to be(true), "expected #{name.inspect} to be droppable"
      end
    end
  end
end
