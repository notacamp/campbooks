# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::MarkThreadRead do
  include ActiveJob::TestHelper

  let(:account) { create(:email_account) }
  let(:thread) { create(:email_thread, email_account: account, subject: "Q3 deck") }

  def message(read:, provider_message_id:)
    create(:email_message, email_account: account, email_thread: thread, provider_message_id: provider_message_id)
      .tap { |m| m.update_columns(read: read, viewed_at: nil) }
  end

  it "marks every unread message read and viewed, syncs the provider, and returns true" do
    unread = message(read: false, provider_message_id: "p-1")
    seen = message(read: true, provider_message_id: "p-2")

    result = nil
    expect { result = described_class.call(thread) }
      .to have_enqueued_job(MarkReadJob).with(account.id, [ "p-1" ])

    expect(result).to be true
    expect(unread.reload).to have_attributes(read: true)
    expect(unread.viewed_at).to be_present
    expect(seen.reload.viewed_at).to be_present # viewed even when it was already read
  end

  it "returns false and enqueues nothing when nothing was unread" do
    message(read: true, provider_message_id: "p-1")

    result = nil
    expect { result = described_class.call(thread) }.not_to have_enqueued_job(MarkReadJob)
    expect(result).to be false
  end

  it "returns false for a nil thread" do
    expect(described_class.call(nil)).to be false
  end
end
