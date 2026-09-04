# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Attention do
  around { |e| travel_to(Time.zone.local(2026, 9, 4, 12, 0, 0)) { e.run } }

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@biz.example") }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return(%w[INBOX])
  end

  def attention = described_class.new(user, now: Time.current)

  # Create a feed item and return it with its person/org.
  def make_email_feed_item(kind:, score: 60.0, age_days: 5, person: nil, contact: nil, **msg_attrs)
    person  ||= create(:person, workspace: workspace)
    contact ||= create(:contact, workspace: workspace, email_account: account, person: person,
                       sender_kind: :person, sender_kind_source: "heuristic",
                       email: "#{person.name.parameterize}@x.example")
    thread = create(:email_thread, email_account: account)
    msg = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                 from_address: contact.email, to_address: account.email_address,
                 provider_folder_id: "INBOX", received_at: age_days.days.ago,
                 skimmed_at: nil, ai_todo_dismissed: false,
                 **msg_attrs)
    thread.update_columns(last_inbound_at: age_days.days.ago, last_outbound_at: nil)

    fi = FeedItem.create!(
      user: user, workspace: workspace, kind: kind, subject: msg,
      dedupe_key: "#{kind}:#{msg.id}", sort_at: msg.received_at, score: score,
      attention: age_days >= 7,
      data: { "age_days" => age_days }
    )
    [ fi, person, contact, thread, msg ]
  end

  def make_doc_feed_item(kind:, score: 90.0, days_late: 10, person: nil, org: nil)
    person ||= create(:person, workspace: workspace)
    org    ||= person.primary_organization
    doc = create(:document, :approved, workspace: workspace,
                 amount_cents: 50_000, currency: "EUR", due_date: days_late.days.ago)
    # Associate the doc with an email from the person.
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     email: "#{person.name.parameterize}@x.example", sender_kind: :person,
                     sender_kind_source: "heuristic")
    thread  = create(:email_thread, email_account: account)
    msg     = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                     from_address: contact.email, to_address: account.email_address,
                     provider_folder_id: "INBOX", received_at: 5.days.ago)
    doc.email_messages << msg

    fi = FeedItem.create!(
      user: user, workspace: workspace, kind: kind, subject: doc,
      dedupe_key: "#{kind}:#{doc.id}", sort_at: days_late.days.ago.in_time_zone, score: score,
      attention: true,
      data: { "days_late" => days_late, "amount_cents" => 50_000, "currency" => "EUR",
              "due_date" => days_late.days.ago.to_date.iso8601 }
    )
    [ fi, person, org, doc ]
  end

  # ── Item maps to sender's person ─────────────────────────────────────────

  it "maps an email message item to the sender's person" do
    _fi, person, = make_email_feed_item(kind: "reply_owed", age_days: 5)
    item = attention.for(person)
    expect(item).not_to be_nil
    expect(item.verb).to eq(:reply)
  end

  # ── Document item maps to org (else person) ───────────────────────────────

  it "maps a document item to the sender's primary org when present" do
    org = create(:organization, workspace: workspace, name: "Fasthost")
    person = create(:person, workspace: workspace, name: "Ivo")
    create(:organization_membership, person: person, organization: org)
    person.reload

    _fi, _p, _org, = make_doc_feed_item(kind: "late_payable", person: person, org: org)
    item = attention.for(org)
    expect(item).not_to be_nil
    expect(item.verb).to eq(:pay)
  end

  it "maps a document item to the person directly when no primary org" do
    person = create(:person, workspace: workspace, name: "Ivo")
    _fi, _p, = make_doc_feed_item(kind: "late_payable", person: person)
    item = attention.for(person)
    expect(item).not_to be_nil
    expect(item.verb).to eq(:pay)
  end

  # ── Best item per counterpart (highest score) ─────────────────────────────

  it "keeps the highest-scoring item when a person has multiple active items" do
    person = create(:person, workspace: workspace, name: "Sofia")
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "sofia@x.example")

    make_email_feed_item(kind: "reply_owed", score: 40.0, person: person, contact: contact, age_days: 5)
    make_email_feed_item(kind: "email_action", score: 85.0, person: person, contact: contact,
                         age_days: 3, ai_action_prompt: "Review the contract.")

    item = attention.for(person)
    expect(item.verb).to eq(:decide)
    expect(item.feed_item.score).to eq(85.0)
  end

  # ── Verb per kind ─────────────────────────────────────────────────────────

  it "reply_owed and reply_reminder → verb :reply" do
    _fi, person, = make_email_feed_item(kind: "reply_owed", age_days: 5)
    expect(attention.for(person)&.verb).to eq(:reply)
  end

  it "follow_up → verb :nudge" do
    # follow_up still_valid? requires the thread to hold_last_word?
    # (last_outbound_at >= last_inbound_at), so set up that scenario.
    _fi, person, contact, thread, = make_email_feed_item(kind: "follow_up", age_days: 8)
    # User replied after the inbound: now the user holds last word.
    thread.update_columns(last_outbound_at: 2.days.ago, last_inbound_at: 8.days.ago,
                          follow_up_dismissed_at: nil)
    expect(attention.for(person)&.verb).to eq(:nudge)
  end

  it "email_action → verb :decide" do
    _fi, person, = make_email_feed_item(kind: "email_action", age_days: 3,
                                        ai_action_prompt: "Please confirm.")
    expect(attention.for(person)&.verb).to eq(:decide)
  end

  it "late_payable → verb :pay" do
    person = create(:person, workspace: workspace)
    _fi, = make_doc_feed_item(kind: "late_payable", person: person)
    expect(attention.for(person)&.verb).to eq(:pay)
  end

  it "late_receivable → verb :chase" do
    person = create(:person, workspace: workspace)
    doc = create(:document, :approved, :revenue_invoice, workspace: workspace,
                 amount_cents: 50_000, currency: "EUR", due_date: 10.days.ago)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     email: "#{person.name.parameterize}@x.example", sender_kind: :person,
                     sender_kind_source: "heuristic")
    thread = create(:email_thread, email_account: account)
    msg    = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                    from_address: contact.email, to_address: account.email_address,
                    provider_folder_id: "INBOX", received_at: 5.days.ago)
    doc.email_messages << msg
    FeedItem.create!(user: user, workspace: workspace, kind: "late_receivable", subject: doc,
                     dedupe_key: "late_receivable:#{doc.id}", sort_at: 10.days.ago, score: 90.0,
                     attention: true, data: { "days_late" => 10, "amount_cents" => 50_000 })
    expect(attention.for(person)&.verb).to eq(:chase)
  end

  # ── wait_days per kind ────────────────────────────────────────────────────

  it "uses age_days data for reply_owed wait_days" do
    _fi, person, = make_email_feed_item(kind: "reply_owed", age_days: 8)
    expect(attention.for(person)&.wait_days).to eq(8)
  end

  it "uses days_late data for late_payable wait_days" do
    person = create(:person, workspace: workspace)
    make_doc_feed_item(kind: "late_payable", person: person, days_late: 14)
    expect(attention.for(person)&.wait_days).to eq(14)
  end

  # ── subject string ────────────────────────────────────────────────────────

  it "subject is the thread display subject for email items" do
    person = create(:person, workspace: workspace)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "p@x.example")
    thread = create(:email_thread, email_account: account, subject: "Q3 kick-off")
    msg = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                 from_address: contact.email, to_address: account.email_address,
                 provider_folder_id: "INBOX", received_at: 5.days.ago, subject: "Q3 kick-off")
    thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)
    FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg,
                     dedupe_key: "reply_owed:#{msg.id}", sort_at: msg.received_at, score: 50.0,
                     attention: false, data: { "age_days" => 5 })

    item = attention.for(person)
    expect(item&.subject).to include("Q3 kick-off")
  end

  it "subject is an amount + date string for money document items" do
    person = create(:person, workspace: workspace)
    make_doc_feed_item(kind: "late_payable", person: person, days_late: 10)
    item = attention.for(person)
    expect(item&.subject).to include("€500.00")
  end

  # ── text only for reply_reminder / email_action / follow_up ──────────────

  it "text is ai_action_prompt for email_action items" do
    _fi, person, = make_email_feed_item(kind: "email_action", age_days: 3,
                                        ai_action_prompt: "Please approve the budget.")
    expect(attention.for(person)&.text).to eq("Please approve the budget.")
  end

  it "text is nil for reply_owed items (no AI prompt)" do
    _fi, person, = make_email_feed_item(kind: "reply_owed", age_days: 5)
    expect(attention.for(person)&.text).to be_nil
  end

  it "text is nil for late_payable items" do
    person = create(:person, workspace: workspace)
    make_doc_feed_item(kind: "late_payable", person: person)
    expect(attention.for(person)&.text).to be_nil
  end

  # ── still_valid? false items are skipped ─────────────────────────────────

  it "skips feed items whose subject is no longer valid" do
    person = create(:person, workspace: workspace)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     sender_kind: :person, sender_kind_source: "heuristic", email: "p@x.example")
    thread  = create(:email_thread, email_account: account)
    msg = create(:email_message, email_account: account, contact: contact, email_thread: thread,
                 from_address: contact.email, to_address: account.email_address,
                 provider_folder_id: "INBOX", received_at: 5.days.ago, skimmed_at: nil,
                 ai_todo_dismissed: false)
    thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)
    FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg,
                     dedupe_key: "reply_owed:#{msg.id}", sort_at: msg.received_at, score: 50.0,
                     attention: false, data: { "age_days" => 5 })

    # Invalidate: mark the message as skimmed.
    msg.update_columns(skimmed_at: Time.current)

    expect(attention.for(person)).to be_nil
  end
end
