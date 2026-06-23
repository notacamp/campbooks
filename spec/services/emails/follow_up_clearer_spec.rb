require "rails_helper"

RSpec.describe Emails::FollowUpClearer do
  let(:account) { create(:email_account) }

  it "clears a pending follow-up on the thread" do
    thread = create(:email_thread, email_account: account,
                    follow_up_expected: true, follow_up_at: 1.day.ago, follow_up_reason: "Confirm date")

    described_class.call(thread)

    thread.reload
    expect(thread.follow_up_expected?).to be(false)
    expect(thread.follow_up_at).to be_nil
    expect(thread.follow_up_reason).to be_nil
  end

  it "is a no-op when no follow-up is pending" do
    thread = create(:email_thread, email_account: account, follow_up_expected: false)
    expect { described_class.call(thread) }.not_to raise_error
  end

  it "tolerates a nil thread" do
    expect { described_class.call(nil) }.not_to raise_error
  end
end
