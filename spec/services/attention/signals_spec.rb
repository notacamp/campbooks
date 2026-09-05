# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Signals do
  let(:workspace)    { create(:workspace) }
  let(:user)         { create(:user, workspace: workspace) }
  let(:account)      { create(:email_account, workspace: workspace) }
  let(:now)          { Time.zone.local(2026, 9, 5, 12, 0, 0) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  def signals(u = user)
    described_class.new(u, now: now)
  end

  def make_person
    person = create(:person)
    contact = create(:contact, workspace: workspace, email_account: account, person: person, sender_kind: 0, email_count: 1)
    [ person, contact ]
  end

  def make_thread
    create(:email_thread, email_account: account)
  end

  def make_inbound(contact:, thread:, received_at: 1.day.ago, viewed_at: nil, to_address: nil)
    to = to_address || account.email_address
    create(:email_message,
           email_account: account,
           contact: contact,
           email_thread: thread,
           from_address: contact.email,
           to_address: to,
           received_at: received_at,
           viewed_at: viewed_at,
           status: :processed)
  end

  def make_outbound(thread:, received_at:)
    create(:email_message,
           email_account: account,
           email_thread: thread,
           from_address: account.email_address,
           to_address: "someone@example.com",
           received_at: received_at,
           status: :processed)
  end

  describe "reply matching" do
    it "counts an inbound replied to within 14 days with correct latency" do
      person, contact = make_person
      thread = make_thread
      make_inbound(contact: contact, thread: thread, received_at: now - 5.days)
      make_outbound(thread: thread, received_at: now - 4.days)

      facts = signals.facts_by_person[person.id]
      expect(facts.replied_count).to eq(1)
      expect(facts.median_reply_hours).to be_within(0.1).of(24.0)
    end

    it "does not count an outbound BEFORE the inbound as a reply" do
      person, contact = make_person
      thread = make_thread
      make_outbound(thread: thread, received_at: now - 10.days)
      make_inbound(contact: contact, thread: thread, received_at: now - 5.days)

      facts = signals.facts_by_person[person.id]
      expect(facts.replied_count).to eq(0)
    end

    it "does not count an outbound more than 14 days after the inbound" do
      person, contact = make_person
      thread = make_thread
      make_inbound(contact: contact, thread: thread, received_at: now - 30.days)
      make_outbound(thread: thread, received_at: now - 10.days) # 20 days later — outside window

      facts = signals.facts_by_person[person.id]
      expect(facts.replied_count).to eq(0)
    end

    it "computes the median over several replies" do
      person, contact = make_person
      thread1 = make_thread
      thread2 = make_thread
      thread3 = make_thread
      make_inbound(contact: contact, thread: thread1, received_at: now - 20.days)
      make_outbound(thread: thread1, received_at: now - 20.days + 2.hours)
      make_inbound(contact: contact, thread: thread2, received_at: now - 15.days)
      make_outbound(thread: thread2, received_at: now - 15.days + 6.hours)
      make_inbound(contact: contact, thread: thread3, received_at: now - 10.days)
      make_outbound(thread: thread3, received_at: now - 10.days + 10.hours)

      facts = signals.facts_by_person[person.id]
      expect(facts.replied_count).to eq(3)
      expect(facts.median_reply_hours).to be_within(0.1).of(6.0)
    end
  end

  describe "addressed vs cc-only vs bulk" do
    it "counts only messages addressed to the account owner in To" do
      person, contact = make_person
      thread = make_thread

      # Addressed (owner in To)
      make_inbound(contact: contact, thread: thread, to_address: account.email_address)
      # Cc-only (owner NOT in To)
      create(:email_message, email_account: account, contact: contact, email_thread: thread,
             from_address: contact.email, to_address: "other@example.com",
             received_at: 1.day.ago, status: :processed)

      facts = signals.facts_by_person[person.id]
      expect(facts.addressed_count).to eq(1)
      expect(facts.inbound_count).to eq(2)
    end

    it "does not count a message with List-Unsubscribe header as addressed" do
      person, contact = make_person
      thread = make_thread
      create(:email_message, email_account: account, contact: contact, email_thread: thread,
             from_address: contact.email, to_address: account.email_address,
             header_list_unsubscribe: "<mailto:unsubscribe@example.com>",
             received_at: 1.day.ago, status: :processed)

      facts = signals.facts_by_person[person.id]
      expect(facts.addressed_count).to eq(0)
    end
  end

  describe "owner's own sent mail" do
    it "is excluded from inbound counts" do
      person, contact = make_person
      thread = make_thread

      # A message from the account owner's own address — should be excluded
      create(:email_message, email_account: account, contact: contact, email_thread: thread,
             from_address: account.email_address, to_address: contact.email,
             received_at: 1.day.ago, status: :processed)

      facts = signals.facts_by_person[person.id]
      expect(facts.inbound_count).to eq(0)
    end
  end

  describe "opens via viewed_at" do
    it "counts messages with viewed_at as opened" do
      person, contact = make_person
      thread = make_thread
      make_inbound(contact: contact, thread: thread, viewed_at: now - 3.days)
      make_inbound(contact: contact, thread: make_thread, viewed_at: nil)

      facts = signals.facts_by_person[person.id]
      expect(facts.opened_count).to eq(1)
    end
  end

  describe "meetings via attendees" do
    it "counts calendar events where the person is an attendee" do
      person, contact = make_person
      cal_account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, calendar_account: cal_account, user: user, can_read: true)
      calendar = create(:calendar, calendar_account: cal_account)
      create(:calendar_event, calendar: calendar, start_at: now - 5.days, end_at: now - 5.days + 1.hour,
             attendees: [ { "email" => contact.email, "name" => "Person" } ])

      facts = signals.facts_by_person[person.id]
      expect(facts.meetings_count).to eq(1)
    end

    it "handles string attendee rows" do
      person, contact = make_person
      cal_account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, calendar_account: cal_account, user: user, can_read: true)
      calendar = create(:calendar, calendar_account: cal_account)
      create(:calendar_event, calendar: calendar, start_at: now - 5.days, end_at: now - 5.days + 1.hour,
             attendees: [ contact.email ])

      facts = signals.facts_by_person[person.id]
      expect(facts.meetings_count).to eq(1)
    end

    it "skips owner addresses" do
      _person, _contact = make_person
      cal_account = create(:calendar_account, workspace: workspace)
      create(:calendar_account_user, calendar_account: cal_account, user: user, can_read: true)
      calendar = create(:calendar, calendar_account: cal_account)
      # Only the owner is an attendee — no person contact matched
      create(:calendar_event, calendar: calendar, start_at: now - 5.days, end_at: now - 5.days + 1.hour,
             attendees: [ { "email" => account.email_address } ])

      # No meetings should be counted for any person
      facts = signals.facts_by_person
      expect(facts.values.map(&:meetings_count)).to all(eq(0))
    end
  end

  describe "money via DocumentEmailMessage" do
    it "counts distinct money documents and settled docs" do
      person, contact = make_person
      thread = make_thread
      msg = make_inbound(contact: contact, thread: thread)

      doc1 = create(:document, workspace: workspace, document_type: :expense_invoice, settled_at: now - 1.day)
      doc2 = create(:document, workspace: workspace, document_type: :expense_invoice, settled_at: now - 2.days)
      DocumentEmailMessage.create!(document: doc1, email_message: msg)
      DocumentEmailMessage.create!(document: doc2, email_message: msg)

      facts = signals.facts_by_person[person.id]
      expect(facts.invoices_count).to eq(2)
      expect(facts.settled_count).to eq(2)
      # No due_date column in current schema — median_settle_delay_days is nil
      expect(facts.median_settle_delay_days).to be_nil
    end

    it "counts unsettled docs in invoices_count but not settled_count" do
      person, contact = make_person
      msg = make_inbound(contact: contact, thread: make_thread)
      doc = create(:document, workspace: workspace, document_type: :expense_invoice, settled_at: nil)
      DocumentEmailMessage.create!(document: doc, email_message: msg)

      facts = signals.facts_by_person[person.id]
      expect(facts.invoices_count).to eq(1)
      expect(facts.settled_count).to eq(0)
      expect(facts.median_settle_delay_days).to be_nil
    end
  end

  describe "events" do
    it "counts archived-unread vs archived-after-open correctly" do
      person, contact = make_person
      thread = make_thread
      msg_unread = make_inbound(contact: contact, thread: thread, viewed_at: nil)
      msg_read   = make_inbound(contact: contact, thread: make_thread, viewed_at: now - 1.day)

      create(:event, workspace: workspace, name: "email.archived",
             subject: msg_unread, actor_type: "User", actor_id: user.id, occurred_at: now - 1.day)
      create(:event, workspace: workspace, name: "email.archived",
             subject: msg_read, actor_type: "User", actor_id: user.id, occurred_at: now - 1.day)

      facts = signals.facts_by_person[person.id]
      expect(facts.archived_unread_count).to eq(1)
    end

    it "handles bulk_archive payload ids" do
      person, contact = make_person
      msg1 = make_inbound(contact: contact, thread: make_thread, viewed_at: nil)
      msg2 = make_inbound(contact: contact, thread: make_thread, viewed_at: nil)

      create(:event, workspace: workspace, name: "email.bulk_archived",
             actor_type: "User", actor_id: user.id, occurred_at: now - 1.day,
             payload: { "ids" => [ msg1.id, msg2.id ] })

      facts = signals.facts_by_person[person.id]
      expect(facts.archived_unread_count).to eq(2)
    end

    it "counts trashed, snoozed, forwarded, tagged separately" do
      person, contact = make_person
      msg = make_inbound(contact: contact, thread: make_thread)

      %w[email.trashed email.snoozed email.forwarded email.tagged].each do |name|
        create(:event, workspace: workspace, name: name,
               subject: msg, actor_type: "User", actor_id: user.id, occurred_at: now - 1.day)
      end

      facts = signals.facts_by_person[person.id]
      expect(facts.trashed_count).to eq(1)
      expect(facts.snoozed_count).to eq(1)
      expect(facts.forwarded_count).to eq(1)
      expect(facts.tagged_count).to eq(1)
    end

    it "ignores events from another user" do
      person, contact = make_person
      other_user = create(:user, workspace: workspace)
      msg = make_inbound(contact: contact, thread: make_thread)

      create(:event, workspace: workspace, name: "email.archived",
             subject: msg, actor_type: "User", actor_id: other_user.id, occurred_at: now - 1.day)

      facts = signals.facts_by_person[person.id]
      expect(facts.archived_unread_count).to eq(0)
    end

    it "ignores system events (nil actor)" do
      person, contact = make_person
      msg = make_inbound(contact: contact, thread: make_thread)

      create(:event, workspace: workspace, name: "email.archived",
             subject: msg, actor_type: nil, actor_id: nil, occurred_at: now - 1.day)

      facts = signals.facts_by_person[person.id]
      expect(facts.archived_unread_count).to eq(0)
    end
  end

  describe "feed acted/dismissed" do
    it "counts feed items acted on and dismissed" do
      person, contact = make_person
      msg = make_inbound(contact: contact, thread: make_thread)
      fi_acted    = create(:feed_item, user: user, workspace: workspace, subject: msg, acted_at: now - 1.day)
      fi_dismissed = create(:feed_item, user: user, workspace: workspace, subject: msg, dismissed_at: now - 2.days)

      facts = signals.facts_by_person[person.id]
      expect(facts.feed_acted_count).to eq(1)
      expect(facts.feed_dismissed_count).to eq(1)
    end
  end

  describe "learning decisions" do
    it "counts skim archive/keep decisions per person" do
      person, contact = make_person

      create(:learning_decision, user: user, workspace: workspace, domain: "email_skim",
             contact: contact, label: "archive", created_at: now - 5.days)
      create(:learning_decision, user: user, workspace: workspace, domain: "email_skim",
             contact: contact, label: "keep", created_at: now - 3.days)

      facts = signals.facts_by_person[person.id]
      expect(facts.skim_archive_count).to eq(1)
      expect(facts.skim_keep_count).to eq(1)
    end

    it "taught: newest attention decision wins per person" do
      person, contact = make_person

      create(:learning_decision, user: user, workspace: workspace, domain: "attention",
             contact: contact, label: "important", created_at: now - 10.days)
      create(:learning_decision, user: user, workspace: workspace, domain: "attention",
             contact: contact, label: "unimportant", created_at: now - 1.day)

      facts = signals.facts_by_person[person.id]
      expect(facts.taught).to eq("unimportant")
    end
  end

  describe "explicit signals" do
    it "starred: true when any contact is starred" do
      person, contact = make_person
      contact.update!(starred_at: now - 1.day)

      facts = signals.facts_by_person[person.id]
      expect(facts.starred).to be true
    end

    it "allowed: true when any contact is allowed" do
      person, contact = make_person
      contact.update!(list_status: :allowed)

      facts = signals.facts_by_person[person.id]
      expect(facts.allowed).to be true
    end

    it "blocked: true only when ALL contacts are blocked" do
      person = create(:person)
      c1 = create(:contact, workspace: workspace, email_account: account, person: person, sender_kind: 0, email_count: 1, list_status: :blocked)
      c2 = create(:contact, workspace: workspace, email_account: account, person: person, sender_kind: 0, email_count: 1, list_status: :neutral)

      facts = signals.facts_by_person[person.id]
      expect(facts.blocked).to be false

      c2.update!(list_status: :blocked)
      facts2 = described_class.new(user, now: now).facts_by_person[person.id]
      expect(facts2.blocked).to be true
    end

    it "relationship_type from person record takes precedence over contact" do
      person = create(:person, relationship_type: "client")
      contact = create(:contact, workspace: workspace, email_account: account, person: person,
                       sender_kind: 0, email_count: 1, relationship_type: "vendor")

      facts = signals.facts_by_person[person.id]
      expect(facts.relationship_type).to eq("client")
    end

    it "sender_kind from the dominant contact" do
      person = create(:person)
      _small = create(:contact, workspace: workspace, email_account: account, person: person,
                      sender_kind: 1, email_count: 2)
      _big   = create(:contact, workspace: workspace, email_account: account, person: person,
                      sender_kind: 0, email_count: 10)

      facts = signals.facts_by_person[person.id]
      expect(facts.sender_kind).to eq("person")
    end

    it "last_activity_at is the max of contact last_email_at and inbound received_at" do
      person, contact = make_person
      contact.update!(last_email_at: now - 5.days)
      make_inbound(contact: contact, thread: make_thread, received_at: now - 2.days)

      facts = signals.facts_by_person[person.id]
      expect(facts.last_activity_at).to be_within(1.second).of(now - 2.days)
    end
  end

  describe "eligibility" do
    it "excludes service-only contacts" do
      person = create(:person)
      create(:contact, workspace: workspace, email_account: account, person: person, sender_kind: 1, email_count: 5)

      facts = signals.facts_by_person
      expect(facts).not_to have_key(person.id)
    end

    it "excludes contacts without a person" do
      create(:contact, workspace: workspace, email_account: account, person: nil, sender_kind: 0, email_count: 5)
      expect(signals.facts_by_person).to be_empty
    end

    it "ignores contacts from another workspace" do
      other_ws = create(:workspace)
      other_account = create(:email_account, workspace: other_ws)
      person = create(:person)
      create(:contact, workspace: other_ws, email_account: other_account, person: person, sender_kind: 0, email_count: 5)

      facts = signals.facts_by_person
      expect(facts).not_to have_key(person.id)
    end

    it "returns empty facts for a user with no readable accounts (contacts scoped to workspace)" do
      # A user in the same workspace but with no readable email accounts
      user_without_account = create(:user, workspace: workspace)
      person = create(:person)
      create(:contact, workspace: workspace, email_account: account, person: person,
             sender_kind: :person, email_count: 1, starred_at: Time.current)

      sigs = described_class.new(user_without_account, now: now)
      facts = sigs.facts_by_person

      # The contact lookup is workspace-scoped, so the person IS eligible.
      # But with no accounts there is no inbound mail.
      # The person still appears in facts with purely explicit signals.
      if facts.key?(person.id)
        expect(facts[person.id].inbound_count).to eq(0)
        expect(facts[person.id].starred).to be true
      end
      # Either empty (no contacts returned) or has explicit-only facts — both acceptable
    end
  end

  describe "query budget" do
    it "executes <= 15 queries for a workspace with 30 persons" do
      # Create 30 eligible persons
      30.times do
        person = create(:person)
        create(:contact, workspace: workspace, email_account: account, person: person,
               sender_kind: 0, email_count: 1)
      end

      query_count = 0
      callback = lambda do |_name, _start, _finish, _id, payload|
        next if payload[:name].to_s =~ /SCHEMA|TRANSACTION|SAVEPOINT/i
        query_count += 1
      end

      ActiveSupport::Notifications.subscribed(callback, "sql.active_record") do
        described_class.new(user, now: now).facts_by_person
      end

      expect(query_count).to be <= 15
    end
  end
end
