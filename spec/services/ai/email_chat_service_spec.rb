require "rails_helper"

RSpec.describe Ai::EmailChatService do
  describe "existing-commitment awareness in the system prompt" do
    let(:thread)   { create(:email_thread) }
    let!(:message) { create(:email_message, email_thread: thread, email_account: thread.email_account) }

    def system_prompt
      described_class.new(thread).send(:system_message)[:content]
    end

    it "tells Scout about a calendar event already created from the thread" do
      create(:calendar_event, source_email_message: message, title: "Project sync")
      prompt = system_prompt
      expect(prompt).to include("already extracted from this thread")
      expect(prompt).to include("Project sync")
    end

    it "tells Scout about a pending reminder from the thread" do
      create(:reminder, source: message, title: "Invoice #42 due")
      prompt = system_prompt
      expect(prompt).to include("Pending reminder")
      expect(prompt).to include("Invoice #42 due")
    end

    it "ignores a cancelled event so Scout may re-create it" do
      create(:calendar_event, :cancelled, source_email_message: message, title: "Scrapped")
      expect(system_prompt).not_to include("already extracted from this thread")
    end

    it "omits the section entirely when the thread has no commitments" do
      expect(system_prompt).not_to include("already extracted from this thread")
    end
  end
end
