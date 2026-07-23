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

  it "reconciles: a card whose record is handled elsewhere is expired on the next run" do
    msg = create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)
    item = active_for(msg)
    expect(item).to be_present

    msg.update!(skimmed_at: Time.current) # addressed in Skim, outside the feed
    Feed::Generator.for_user(user)

    item.reload
    expect(item).not_to be_active
    expect(item).to be_expired
    expect(item.acted_at).to be_nil # system expiry, not a user action
  end

  it "revives an expired card when its record qualifies again" do
    msg = create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)
    item = active_for(msg)

    msg.update!(skimmed_at: Time.current)
    Feed::Generator.for_user(user)
    expect(item.reload).to be_expired

    msg.update!(skimmed_at: nil) # e.g. restored from Skim
    Feed::Generator.for_user(user)

    expect(item.reload).to be_active
  end

  it "anchors an un-analyzed follow-up on the send moment, stable across runs" do
    thread = create(:email_thread, email_account: account,
                    last_outbound_at: 4.days.ago, last_inbound_at: 5.days.ago)
    create(:email_message, email_account: account, email_thread: thread,
           from_address: "me@biz.example", received_at: 4.days.ago)
    create(:email_message, email_account: account, email_thread: thread,
           from_address: "dana@acme.com", received_at: 5.days.ago)

    Feed::Generator.for_user(user)
    item = user.feed_items.find_by!(dedupe_key: "follow_up:#{thread.id}")
    expect(item.sort_at).to be_within(1.second).of(thread.last_outbound_at)

    Feed::Generator.new(user, now: 2.hours.from_now).call # a later run must not re-date it

    expect(item.reload.sort_at).to be_within(1.second).of(thread.last_outbound_at)
  end

  it "fences out fossils: a candidate decayed below MIN_SCORE is not materialized" do
    create(:email_message, email_account: account, ai_action_prompt: "Reply",
           ai_suggested_actions: [ { "tool" => "draft_reply" } ], received_at: 200.days.ago)

    Feed::Generator.for_user(user)

    expect(user.feed_items).to be_empty
  end

  it "never resurrects a dismissed card" do
    msg = create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user)
    active_for(msg).dismiss!

    Feed::Generator.for_user(user) # the message still qualifies, but the user dismissed it

    expect(user.feed_items.find_by(subject: msg)).to be_dismissed
    expect(user.feed_items.active.where(subject: msg)).to be_empty
  end

  describe "conversation-fragment collapse (broken threading / mojibake subjects)" do
    # One conversation whose replies landed in SEPARATE EmailThread rows — the
    # reply hop re-encoded the accented subject, so subject-keyed threading (and
    # the provider) treated each variant as a new conversation.
    def fragment(subject_line, from: "Paulo Lobo <paulo@corretor.example>", received:)
      thread = EmailThread.create!(subject: subject_line, email_account: account)
      create(:email_message, email_account: account, email_thread: thread,
             from_address: from, subject: subject_line,
             ai_action_prompt: "Reply", received_at: received)
    end

    it "keeps one card per conversation, preferring the newest fragment" do
      fragment("seguro de saúde", received: 3.hours.ago)
      newest = fragment("RE: FW: seguro de saÃƒÂºde", received: 1.hour.ago)

      Feed::Generator.for_user(user)

      items = user.feed_items.active
      expect(items.count).to eq(1)
      expect(items.first.subject).to eq(newest)
    end

    it "keeps different senders' same-subject conversations separate" do
      fragment("Proposta", from: "a@one.example", received: 2.hours.ago)
      fragment("Proposta", from: "b@two.example", received: 1.hour.ago)

      Feed::Generator.for_user(user)

      expect(user.feed_items.active.count).to eq(2)
    end

    it "never conversation-claims a blank subject" do
      fragment("", received: 2.hours.ago)
      fragment("", received: 1.hour.ago)

      Feed::Generator.for_user(user)

      expect(user.feed_items.active.count).to eq(2)
    end
  end
end
