# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tools::BulkProcessAi do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    Current.acting_user = user
    Current.workspace = workspace
    create(:email_account_user, :manager, user: user, email_account: account)
    allow(EmailProcessJob).to receive(:set).and_return(EmailProcessJob)
    allow(EmailProcessJob).to receive(:perform_later)
  end

  after { Current.reset }

  def make_thread_with_messages(count)
    thread = create(:email_thread, email_account: account)
    count.times { create(:email_message, email_thread: thread, email_account: account, status: :fetched) }
    thread
  end

  it "enqueues EmailProcessJob for each message and returns the count" do
    thread = make_thread_with_messages(3)
    message_ids = thread.email_messages.pluck(:id).map(&:to_s)

    result = described_class.call(email_ids: message_ids)

    expect(EmailProcessJob).to have_received(:perform_later).exactly(3).times
    expect(result[:count]).to eq(3)
    expect(result[:capped]).to be(false)
  end

  it "uses BACKGROUND_PRIORITY so bulk jobs don't starve interactive work" do
    thread = make_thread_with_messages(2)
    message_ids = thread.email_messages.pluck(:id).map(&:to_s)

    described_class.call(email_ids: message_ids)

    expect(EmailProcessJob).to have_received(:set).with(priority: ApplicationJob::BACKGROUND_PRIORITY).at_least(:once)
  end

  describe "anti-drain cap (MAX_BULK_AI)" do
    it "processes at most MAX_BULK_AI messages per call" do
      stub_const("Tools::BulkProcessAi::MAX_BULK_AI", 2)
      thread = make_thread_with_messages(5)
      message_ids = thread.email_messages.pluck(:id).map(&:to_s)

      result = described_class.call(email_ids: message_ids)

      expect(EmailProcessJob).to have_received(:perform_later).exactly(2).times
      expect(result[:count]).to eq(2)
      expect(result[:total_count]).to eq(5)
      expect(result[:capped]).to be(true)
    end

    it "is not capped when message count is exactly MAX_BULK_AI" do
      stub_const("Tools::BulkProcessAi::MAX_BULK_AI", 3)
      thread = make_thread_with_messages(3)
      message_ids = thread.email_messages.pluck(:id).map(&:to_s)

      result = described_class.call(email_ids: message_ids)

      expect(result[:capped]).to be(false)
      expect(result[:count]).to eq(3)
    end

    it "resets status to :fetched before enqueuing (for re-processing)" do
      thread = make_thread_with_messages(2)
      # Simulate messages that have already been processed — reset should still work
      thread.email_messages.update_all(status: :processed)
      message_ids = thread.email_messages.pluck(:id).map(&:to_s)

      described_class.call(email_ids: message_ids)

      thread.email_messages.each do |msg|
        expect(msg.reload.status).to eq("fetched")
      end
    end
  end
end
