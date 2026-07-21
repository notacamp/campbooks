require "rails_helper"

RSpec.describe Emails::ThreadTagRemover do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace, provider: :zoho) }
  let(:tag) { create(:tag, workspace: workspace, name: "Finance") }

  describe ".call" do
    it "removes a local tag from every message in the thread that carries it" do
      thread = create(:email_thread, email_account: account)
      msg1 = create(:email_message, email_account: account, email_thread: thread)
      msg2 = create(:email_message, email_account: account, email_thread: thread)
      msg1.email_message_tags.create!(tag: tag)
      msg2.email_message_tags.create!(tag: tag)

      described_class.call(message: msg1, tag: tag)

      expect(msg1.reload.tags).not_to include(tag)
      expect(msg2.reload.tags).not_to include(tag)
    end

    it "only touches messages that carry the tag" do
      thread = create(:email_thread, email_account: account)
      tagged   = create(:email_message, email_account: account, email_thread: thread)
      untagged = create(:email_message, email_account: account, email_thread: thread)
      other_tag = create(:tag, workspace: workspace, name: "Other")
      tagged.email_message_tags.create!(tag: tag)
      untagged.email_message_tags.create!(tag: other_tag)

      described_class.call(message: tagged, tag: tag)

      expect(tagged.reload.tags).not_to include(tag)
      expect(untagged.reload.tags).to include(other_tag)
    end

    it "continues removing from remaining messages when the provider raises for one" do
      external_tag = create(:tag, :external, workspace: workspace, email_account: account)
      thread = create(:email_thread, email_account: account)
      msg1 = create(:email_message, email_account: account, email_thread: thread)
      msg2 = create(:email_message, email_account: account, email_thread: thread)
      msg1.email_message_tags.create!(tag: external_tag)
      msg2.email_message_tags.create!(tag: external_tag)

      service_instance = instance_double(Zoho::LabelAssignmentService)
      allow(Zoho::LabelAssignmentService).to receive(:new).and_return(service_instance)
      call_count = 0
      allow(service_instance).to receive(:remove) do
        call_count += 1
        raise Zoho::LabelAssignmentService::Error, "network error" if call_count == 1
      end

      expect(Rails.logger).to receive(:error).with(/\[ThreadTagRemover\] Remove failed/)

      described_class.call(message: msg1, tag: external_tag)

      # Second message's remove was still attempted despite the first error
      expect(call_count).to eq(2)
    end

    it "falls back to message-only removal when the message has no thread" do
      msg = create(:email_message, email_account: account, email_thread: nil)
      msg.email_message_tags.create!(tag: tag)

      described_class.call(message: msg, tag: tag)

      expect(msg.reload.tags).not_to include(tag)
    end
  end
end
