# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::ReplyOwed do
  around { |e| travel_to(Time.zone.local(2026, 9, 4, 12, 0, 0)) { e.run } }

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@biz.example") }
  subject(:source) { described_class.new(user) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return(%w[INBOX])
  end

  def make_message(received_at: 5.days.ago, **attrs)
    person  = attrs.delete(:person) || create(:person, workspace: workspace)
    contact = attrs.delete(:contact) || create(:contact, workspace: workspace, email_account: account,
                                                          person: person, sender_kind: :person,
                                                          sender_kind_source: "heuristic",
                                                          email: "#{person.name.parameterize}@x.example")
    thread  = attrs.delete(:thread) || create(:email_thread, email_account: account)
    msg = create(:email_message, {
      email_account: account, contact: contact, email_thread: thread,
      from_address: contact.email, to_address: account.email_address,
      provider_folder_id: "INBOX", received_at: received_at,
      skimmed_at: nil, ai_todo_dismissed: false
    }.merge(attrs))
    thread.update_columns(last_inbound_at: received_at, last_outbound_at: nil)
    [ msg, contact, thread, person ]
  end

  def candidate_ids = source.candidates.map { |c| c[:subject].id }

  # ── Age gate ─────────────────────────────────────────────────────────────

  it "includes a message aged 3+ days" do
    msg, contact = make_message(received_at: 3.days.ago)
    contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).to include(msg.id)
  end

  it "excludes a message younger than 3 days" do
    msg, contact = make_message(received_at: 2.days.ago)
    contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).not_to include(msg.id)
  end

  it "excludes stale messages older than MAX_AGE" do
    stale, = make_message(received_at: (described_class::MAX_AGE + 1.day).ago)
    expect(candidate_ids).not_to include(stale.id)
  end

  # ── Self-sent gate ────────────────────────────────────────────────────────

  it "excludes mail you sent (no contact means it is from the owner)" do
    thread = create(:email_thread, email_account: account)
    sent = create(:email_message, email_account: account, contact: nil, email_thread: thread,
                  from_address: account.email_address, provider_folder_id: "INBOX",
                  received_at: 5.days.ago, skimmed_at: nil, ai_todo_dismissed: false)
    thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)
    expect(candidate_ids).not_to include(sent.id)
  end

  # ── Thread answered gate ─────────────────────────────────────────────────

  it "excludes messages whose thread you already replied to (holds last word)" do
    msg, _, thread = make_message(received_at: 5.days.ago)
    thread.update_columns(last_outbound_at: 3.days.ago, last_inbound_at: 5.days.ago)
    expect(candidate_ids).not_to include(msg.id)
  end

  # ── Broadcast sender gate ─────────────────────────────────────────────────

  it "excludes a broadcast sender (newsletter / unsubscribe header)" do
    msg, = make_message(received_at: 5.days.ago,
                        header_list_unsubscribe: "<mailto:unsub@newsletter.example>")
    # Add a starred relationship so the only thing excluding is the broadcast gate.
    msg.contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).not_to include(msg.id)
  end

  # ── Unclassified sender gate ─────────────────────────────────────────────

  it "excludes an unclassified sender (sender_kind_source nil)" do
    person = create(:person, workspace: workspace)
    unclassified = create(:contact, workspace: workspace, email_account: account, person: person,
                          sender_kind: :person, sender_kind_source: nil,
                          email: "unknown@x.example", starred_at: Time.current)
    msg, = make_message(contact: unclassified, person: person, received_at: 5.days.ago)
    expect(candidate_ids).not_to include(msg.id)
  end

  # ── Cc-only gate ─────────────────────────────────────────────────────────

  it "excludes messages you were only CC'd on" do
    msg, = make_message(received_at: 5.days.ago,
                        to_address: "someone@else.example",
                        cc_address: account.email_address)
    msg.contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).not_to include(msg.id)
  end

  it "includes messages where you are in To even if also in Cc" do
    msg, = make_message(received_at: 5.days.ago,
                        to_address: account.email_address,
                        cc_address: "someone@else.example")
    msg.contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).to include(msg.id)
  end

  # ── Established relationship signals ─────────────────────────────────────

  it "excludes a stranger (no outbound, not starred/allowed/VIP)" do
    stranger, = make_message(received_at: 5.days.ago)
    # contact is heuristic but not starred/allowed/VIP and thread has no outbound
    expect(candidate_ids).not_to include(stranger.id)
  end

  it "includes via starred contact" do
    msg, contact = make_message(received_at: 5.days.ago)
    contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).to include(msg.id)
  end

  it "includes via allowed contact" do
    msg, contact = make_message(received_at: 5.days.ago)
    contact.update_column(:list_status, Contact.list_statuses[:allowed])
    expect(candidate_ids).to include(msg.id)
  end

  it "includes via VIP relationship label" do
    person = create(:person, workspace: workspace, relationship_type: "client")
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "client@x.example")
    msg, = make_message(person: person, contact: contact, received_at: 5.days.ago)
    expect(candidate_ids).to include(msg.id)
  end

  it "includes via prior outbound on the thread" do
    msg, _, thread = make_message(received_at: 5.days.ago)
    thread.update_columns(last_outbound_at: 10.days.ago, last_inbound_at: 5.days.ago)
    expect(candidate_ids).to include(msg.id)
  end

  # ── Blocked contact gate ─────────────────────────────────────────────────

  it "excludes a blocked contact (not admitted)" do
    msg, contact = make_message(received_at: 5.days.ago)
    contact.update_columns(starred_at: Time.current, list_status: Contact.list_statuses[:blocked])
    expect(candidate_ids).not_to include(msg.id)
  end

  # ── Inbox / archived gate ─────────────────────────────────────────────────

  it "excludes an archived message (not in inbox)" do
    msg, _, _, = make_message(received_at: 5.days.ago, provider_folder_id: "ARCHIVE")
    msg.contact.update_columns(starred_at: Time.current)
    expect(candidate_ids).not_to include(msg.id)
  end

  # ── One candidate per thread ──────────────────────────────────────────────

  it "collapses multiple messages in the same thread to the oldest one" do
    thread = create(:email_thread, email_account: account)
    person = create(:person, workspace: workspace)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "t@x.example",
                     starred_at: Time.current)
    older, = make_message(contact: contact, person: person, thread: thread, received_at: 7.days.ago)
    newer, = make_message(contact: contact, person: person, thread: thread, received_at: 4.days.ago)
    thread.update_columns(last_inbound_at: 4.days.ago, last_outbound_at: nil)

    candidates = source.candidates
    subject_ids = candidates.map { |c| c[:subject].id }
    expect(subject_ids.count { |id| [ older.id, newer.id ].include?(id) }).to eq(1)
    _ = newer # suppress unused warning
  end

  # ── data keys and attention flag ─────────────────────────────────────────

  it "sets data keys and raises attention after 7 days" do
    msg, contact = make_message(received_at: 7.days.ago)
    contact.update_columns(starred_at: Time.current)

    candidate = source.candidates.find { |c| c[:subject].id == msg.id }
    expect(candidate[:attention]).to be true
    expect(candidate[:data]["reason"]).to eq("no_reply")
    expect(candidate[:data]["age_days"]).to eq(7)
    expect(candidate[:data]["since"]).to be_present
    expect(candidate[:dedupe_key]).to eq("reply_owed:#{msg.id}")
  end

  it "does not set attention before 7 days" do
    msg, contact = make_message(received_at: 5.days.ago)
    contact.update_columns(starred_at: Time.current)

    candidate = source.candidates.find { |c| c[:subject].id == msg.id }
    expect(candidate[:attention]).to be false
  end

  # ── still_valid? ─────────────────────────────────────────────────────────

  describe "#still_valid?" do
    def item_double = double("item", data: {})

    it "drops when the message is skimmed" do
      msg, contact, thread = make_message(received_at: 5.days.ago)
      contact.update_columns(starred_at: Time.current)
      msg.update_columns(skimmed_at: Time.current)
      expect(source.still_valid?(item_double, msg.reload)).to be false
    end

    it "drops when the thread holds last word (answered)" do
      msg, contact, thread = make_message(received_at: 5.days.ago)
      contact.update_columns(starred_at: Time.current)
      thread.update_columns(last_outbound_at: 3.days.ago)
      msg.reload
      expect(source.still_valid?(item_double, msg)).to be false
    end

    it "drops when the message is no longer in the inbox" do
      msg, contact = make_message(received_at: 5.days.ago)
      contact.update_columns(starred_at: Time.current)
      msg.update_columns(provider_folder_id: "ARCHIVE")
      expect(source.still_valid?(item_double, msg.reload)).to be false
    end

    it "holds while nothing has changed" do
      msg, contact = make_message(received_at: 5.days.ago)
      contact.update_columns(starred_at: Time.current)
      expect(source.still_valid?(item_double, msg)).to be true
    end

    it "drops when subject is nil" do
      expect(source.still_valid?(item_double, nil)).to be false
    end
  end

  # ── Generator priority: reply_reminder beats reply_owed ──────────────────

  it "reply_reminder claims a high-priority message before reply_owed can" do
    # ReplyReminder's aged_scope requires ai_priority: :high OR ai_suggested_actions
    # containing draft_reply. A message with ai_priority: :high AND an established
    # relationship qualifies for both sources — ReplyReminder wins because it runs first.
    person = create(:person, workspace: workspace)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "r@x.example")
    thread = create(:email_thread, email_account: account)
    msg = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                 from_address: contact.email, to_address: account.email_address,
                 provider_folder_id: "INBOX", received_at: 5.days.ago,
                 ai_priority: :high, ai_action_prompt: "Needs a decision by Friday.",
                 ai_todo_dismissed: false)
    # Established relationship via prior outbound (reply_owed would also claim this).
    thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: 10.days.ago)

    Feed::Generator.for_user(user)

    items = user.feed_items.active.where(subject: msg)
    expect(items.count).to eq(1)
    expect(items.first.kind).to eq("reply_reminder")
  end
end
