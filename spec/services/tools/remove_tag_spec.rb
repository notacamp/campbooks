require "rails_helper"

RSpec.describe Tools::RemoveTag do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:tag) { create(:tag, workspace: workspace, name: "Suppliers") }

  describe ".call" do
    it "removes the tag from every message in the thread" do
      thread = create(:email_thread, email_account: account)
      msg1 = create(:email_message, email_account: account, email_thread: thread)
      msg2 = create(:email_message, email_account: account, email_thread: thread)
      msg1.email_message_tags.create!(tag: tag)
      msg2.email_message_tags.create!(tag: tag)

      result = described_class.call(msg1, "tag_name" => "suppliers")

      expect(result).to eq(tag)
      expect(msg1.reload.tags).not_to include(tag)
      expect(msg2.reload.tags).not_to include(tag)
    end

    it "does not match a same-named tag belonging to a different workspace" do
      other_workspace = create(:workspace)
      other_account   = create(:email_account, workspace: other_workspace)
      other_tag       = create(:tag, workspace: other_workspace, name: "Suppliers")

      msg = create(:email_message, email_account: account)
      msg.email_message_tags.create!(tag: other_tag)

      result = described_class.call(msg, "tag_name" => "suppliers")

      expect(result).to be_nil
      expect(msg.reload.tags).to include(other_tag)
    end

    it "returns nil when the tag is not found in the workspace" do
      msg = create(:email_message, email_account: account)

      result = described_class.call(msg, "tag_name" => "nonexistent")

      expect(result).to be_nil
    end

    it "works for a message with no thread (falls back to message-only removal)" do
      msg = create(:email_message, email_account: account, email_thread: nil)
      msg.email_message_tags.create!(tag: tag)

      result = described_class.call(msg, "tag_name" => "suppliers")

      expect(result).to eq(tag)
      expect(msg.reload.tags).not_to include(tag)
    end
  end
end
