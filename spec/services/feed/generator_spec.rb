require "rails_helper"

RSpec.describe Feed::Generator do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "me@biz.example") }

  before { create(:email_account_user, user: user, email_account: account) }

  def active_for(subject)
    user.feed_items.active.find_by(subject: subject)
  end

  it "materializes one card per actionable record and is idempotent" do
    create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 2.hours.ago)

    first = Feed::Generator.for_user(user)
    second = Feed::Generator.for_user(user)

    expect(first).to eq(2)
    expect(second).to eq(2)
    expect(user.feed_items.active.count).to eq(2)
  end

  it "collapses multiple actionable messages in one thread into a single card" do
    thread = EmailThread.create!(subject: "Quote request", email_account: account)
    create(:email_message, email_account: account, email_thread: thread, ai_action_prompt: "Reply", received_at: 3.hours.ago)
    create(:email_message, email_account: account, email_thread: thread, ai_action_prompt: "Reply", received_at: 1.hour.ago)

    Feed::Generator.for_user(user)

    items = user.feed_items.active
    expect(items.count).to eq(1)
    expect(items.first.data["thread_count"]).to eq(2)
  end

  it "assigns each subject to a single kind by source priority (reminder beats action)" do
    aged = create(:email_message, email_account: account, ai_action_prompt: "Needs a reply",
                  ai_suggested_actions: [ { "tool" => "draft_reply" } ], received_at: 9.days.ago)

    Feed::Generator.for_user(user)

    items = user.feed_items.active.where(subject: aged)
    expect(items.count).to eq(1)
    expect(items.first.kind).to eq("reply_reminder")
  end

  it "reconciles: a card whose record is handled elsewhere is resolved on the next run" do
    msg = create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)
    item = active_for(msg)
    expect(item).to be_present

    msg.update!(skimmed_at: Time.current) # addressed in Skim, outside the feed
    Feed::Generator.for_user(user)

    expect(item.reload).not_to be_active
  end

  it "never resurrects a dismissed card" do
    msg = create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)
    active_for(msg).dismiss!

    Feed::Generator.for_user(user) # the message still qualifies, but the user dismissed it

    expect(user.feed_items.find_by(subject: msg)).to be_dismissed
    expect(user.feed_items.active.where(subject: msg)).to be_empty
  end
end
