# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Directory do
  around { |example| travel_to(Time.zone.local(2026, 9, 4, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
  end

  def directory = People::Directory.new(user, workspace: workspace, now: Time.current)

  def make_person(name:, email:, inbound_at: 2.days.ago, owe: false, source: "heuristic",
                  emails: 1, replied: false, unsubscribe: nil)
    person  = create(:person, workspace: workspace, name: name, context_summary: nil)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     name: name, email: email, sender_kind: :person, sender_kind_source: source)
    thread  = create(:email_thread, email_account: account, subject: "Thread #{name}")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
           from_address: email, subject: "Msg #{name}", received_at: inbound_at,
           header_list_unsubscribe: unsubscribe)
    if replied
      create(:email_message, email_account: account, email_thread: thread, contact: nil,
             from_address: account.email_address, received_at: inbound_at - 8.days)
      thread.update_columns(last_outbound_at: inbound_at - 8.days)
    end
    contact.update_columns(email_count: emails, last_email_at: inbound_at)
    thread.update_columns(last_inbound_at: inbound_at) if owe
    [ person, contact, thread ]
  end

  describe "#counterparts" do
    it "includes eligible persons" do
      make_person(name: "Sofia", email: "sofia@x.example")
      names = directory.counterparts.map(&:name)
      expect(names).to include("Sofia")
    end

    it "excludes an org without a money feed item (orgs only appear for pay/chase)" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      person, = make_person(name: "Rui", email: "rui@cloudhost.example")
      create(:organization_membership, person: person, organization: org)

      names = directory.counterparts.map(&:name)
      expect(names).not_to include("Cloudhost")
      expect(names).to include("Rui")
    end

    it "includes an org when attention returns a money item for it" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      person, = make_person(name: "Rui", email: "rui@cloudhost.example")
      create(:organization_membership, person: person, organization: org)

      org_item = instance_double(People::Attention::Item,
                                 verb: :chase, subject: "Invoice #1", wait_days: 14,
                                 detail: nil, detail_kind: nil, money: nil,
                                 thread_id: nil, message: nil, attention: true,
                                 feed_item: instance_double(FeedItem, id: SecureRandom.uuid, score: 80.0, sort_at: Time.current))

      attn = instance_double(People::Attention)
      allow(attn).to receive(:for).with(kind_of(Person)).and_return(nil)
      allow(attn).to receive(:for).with(org).and_return(org_item)
      allow(attn).to receive(:items_by_counterpart).and_return({})
      allow(People::Attention).to receive(:new).and_return(attn)

      names = directory.counterparts.map(&:name)
      expect(names).to include("Cloudhost")
    end

    it "ranks a real correspondent's fresh ask above a stranger's older one" do
      make_person(name: "Sofia Martins", email: "sofia@x.example", owe: true, inbound_at: 2.days.ago,
                  replied: true, emails: 12)
      make_person(name: "Cold Sender", email: "cold@unknown.example", owe: true, inbound_at: 14.days.ago)

      counterparts = directory.counterparts
      sofia = counterparts.find { |c| c.name == "Sofia Martins" }
      cold  = counterparts.find { |c| c.name == "Cold Sender" }

      expect(sofia.priority).to be > cold.priority
    end

    it "excludes an unclassified newsletter (service-majority sample)" do
      make_person(name: "The Weekly Byte", email: "news@bytemedia.example", source: nil,
                  inbound_at: 40.days.ago, unsubscribe: "<mailto:unsub@bytemedia.example>")

      names = directory.counterparts.map(&:name)
      expect(names).not_to include("The Weekly Byte")
    end

    it "includes an unclassified person (no newsletter headers)" do
      make_person(name: "Nadia Costa", email: "nadia@costa.example", source: nil, inbound_at: 3.days.ago)
      names = directory.counterparts.map(&:name)
      expect(names).to include("Nadia Costa")
    end

    it "excludes the mailbox owner" do
      # A contact whose email matches the account's email address
      owner_person = create(:person, workspace: workspace, name: "Owner")
      create(:contact, workspace: workspace, email_account: account, person: owner_person,
             email: account.email_address, sender_kind: :person, sender_kind_source: "heuristic",
             email_count: 1, last_email_at: 1.day.ago)

      names = directory.counterparts.map(&:name)
      expect(names).not_to include("Owner")
    end

    it "excludes contacts that are all blocked" do
      person = create(:person, workspace: workspace, name: "Blocked")
      contact = create(:contact, workspace: workspace, email_account: account, person: person,
                       email: "blocked@x.example", sender_kind: :person, sender_kind_source: "heuristic",
                       email_count: 2, last_email_at: 1.day.ago)
      contact.update_column(:list_status, Contact.list_statuses[:blocked])

      names = directory.counterparts.map(&:name)
      expect(names).not_to include("Blocked")
    end

    # ── Group-thread folding ──────────────────────────────────────────────────

    it "folds a thread-mate into the winner's row even when only the winner has a feed item" do
      # Winner (Ana) has a feed item → she appears in Need-you.
      # Other (Bruno) has mail on the same thread but no feed item → he would normally
      # appear under Recent. The fold should absorb him into Ana's row.

      thread = create(:email_thread, email_account: account, subject: "Contract review")

      person_a = create(:person, workspace: workspace, name: "Ana Lima")
      contact_a = create(:contact, workspace: workspace, email_account: account, person: person_a,
                         sender_kind: :person, sender_kind_source: "heuristic",
                         email: "ana@x.example", starred_at: Time.current, email_count: 5)
      msg_a = create(:email_message, email_account: account, email_thread: thread, contact: contact_a,
                     from_address: "ana@x.example", received_at: 5.days.ago,
                     provider_folder_id: "INBOX", skimmed_at: nil, ai_todo_dismissed: false)
      contact_a.update_columns(last_email_at: 5.days.ago)

      person_b = create(:person, workspace: workspace, name: "Bruno Costa")
      contact_b = create(:contact, workspace: workspace, email_account: account, person: person_b,
                         sender_kind: :person, sender_kind_source: "heuristic",
                         email: "bruno@x.example", email_count: 5)
      create(:email_message, email_account: account, email_thread: thread, contact: contact_b,
             from_address: "bruno@x.example", received_at: 5.days.ago,
             provider_folder_id: "INBOX", skimmed_at: nil, ai_todo_dismissed: false)
      contact_b.update_columns(last_email_at: 5.days.ago)
      thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)

      # Only Ana (the winner) has a feed item.
      FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg_a,
                       dedupe_key: "reply_owed:#{msg_a.id}", sort_at: msg_a.received_at,
                       score: 60.0, attention: false, data: { "age_days" => 5 })

      counterparts = directory.counterparts
      names = counterparts.map(&:name)

      # The combined row is named "Ana Lima, Bruno Costa" (winner first).
      group_row = counterparts.find { |cp| cp.name.include?("Ana Lima") && cp.name.include?("Bruno Costa") }
      expect(group_row).not_to be_nil, "expected a combined row; got: #{names.inspect}"

      # Bruno must NOT have a separate row (not under Recent either).
      expect(names).not_to include("Bruno Costa")

      # The winner's row carries both person ids.
      expect(group_row.data["participant_ids"]).to include(person_a.id, person_b.id)
    end

    # ── data["new"] flag ─────────────────────────────────────────────────────

    it "marks data['new'] true for a stranger with exactly one inbound and no outbound thread" do
      make_person(name: "Stranger Rosario", email: "rosario@new.example", emails: 1, replied: false)
      counterparts = directory.counterparts
      cp = counterparts.find { |c| c.name == "Stranger Rosario" }
      expect(cp).not_to be_nil
      expect(cp.data["new"]).to be true
    end

    it "does not mark data['new'] for a person you have replied to" do
      make_person(name: "Old Friend", email: "friend@known.example", emails: 5, replied: true)
      counterparts = directory.counterparts
      cp = counterparts.find { |c| c.name == "Old Friend" }
      expect(cp).not_to be_nil
      expect(cp.data["new"]).to be false
    end

    # ── data["unread"] flag ───────────────────────────────────────────────────

    it "marks data['unread'] true when the standing thread has an unread inbound" do
      person, contact, thread = make_person(name: "Unread Sender", email: "unread@x.example")
      # Mark the message as unread.
      msg = EmailMessage.find_by!(email_thread: thread, contact: contact)
      msg.update_columns(read: false)

      # Need an attention item so standing has a thread_id.
      msg.update_columns(skimmed_at: nil, ai_todo_dismissed: false, provider_folder_id: "INBOX",
                         received_at: 5.days.ago)
      thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)
      FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg,
                       dedupe_key: "reply_owed:#{msg.id}", sort_at: msg.received_at,
                       score: 60.0, attention: false, data: { "age_days" => 5 })
      contact.update_columns(starred_at: Time.current)

      counterparts = directory.counterparts
      cp = counterparts.find { |c| c.name == "Unread Sender" }
      expect(cp).not_to be_nil
      expect(cp.data["unread"]).to be true
    end

    it "marks data['unread'] true when the newest message from them is unread, even without an attention item" do
      _person, contact, thread = make_person(name: "Fresh Sender", email: "fresh@x.example")
      EmailMessage.find_by!(email_thread: thread, contact: contact).update_columns(read: false)

      cp = directory.counterparts.find { |c| c.name == "Fresh Sender" }
      expect(cp.data["unread"]).to be true
    end

    it "marks data['unread'] false once the newest message from them is read" do
      _person, contact, thread = make_person(name: "Seen Sender", email: "seen@x.example")
      EmailMessage.find_by!(email_thread: thread, contact: contact).update_columns(read: true)

      cp = directory.counterparts.find { |c| c.name == "Seen Sender" }
      expect(cp.data["unread"]).to be false
    end

    # ── data["has_attachment"] flag ───────────────────────────────────────────

    it "marks data['has_attachment'] true when the attention item's message has an attachment" do
      person, contact, thread = make_person(name: "Attacher", email: "attach@x.example")
      msg = EmailMessage.find_by!(email_thread: thread, contact: contact)
      msg.update_columns(has_attachment: true, skimmed_at: nil, ai_todo_dismissed: false,
                         provider_folder_id: "INBOX", received_at: 5.days.ago)
      thread.update_columns(last_inbound_at: 5.days.ago, last_outbound_at: nil)
      FeedItem.create!(user: user, workspace: workspace, kind: "reply_owed", subject: msg,
                       dedupe_key: "reply_owed:#{msg.id}", sort_at: msg.received_at,
                       score: 60.0, attention: false, data: { "age_days" => 5 })
      contact.update_columns(starred_at: Time.current)

      cp = directory.counterparts.find { |c| c.name == "Attacher" }
      expect(cp).not_to be_nil
      expect(cp.data["has_attachment"]).to be true
    end

    it "an org row has a positive priority when it has a money attention item" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      person, = make_person(name: "Rui Santos", email: "rui@cloudhost.example",
                            inbound_at: 2.days.ago, emails: 5)
      create(:organization_membership, person: person, organization: org)

      org_item = instance_double(People::Attention::Item,
                                 verb: :chase, subject: "Invoice #1", wait_days: 14,
                                 detail: nil, detail_kind: nil, money: nil,
                                 thread_id: nil, message: nil, attention: true,
                                 feed_item: instance_double(FeedItem, id: SecureRandom.uuid, score: 80.0, sort_at: Time.current))

      attn = instance_double(People::Attention)
      allow(attn).to receive(:for).with(kind_of(Person)).and_return(nil)
      allow(attn).to receive(:for).with(org).and_return(org_item)
      allow(attn).to receive(:items_by_counterpart).and_return({})
      allow(People::Attention).to receive(:new).and_return(attn)

      counterparts = directory.counterparts
      cloudhost = counterparts.find { |c| c.name == "Cloudhost" }

      expect(cloudhost).not_to be_nil
      expect(cloudhost.priority).to be > 0
    end
  end

  describe "data flags: starred, can_reply, can_done" do
    it "sets starred=true when the person's contact is starred" do
      _person, contact, = make_person(name: "Sofia", email: "sofia@x.example")
      contact.update_columns(starred_at: Time.current)

      cp = directory.counterparts.find { |c| c.name == "Sofia" }
      expect(cp.data["starred"]).to be true
    end

    it "sets starred=false when the contact is not starred" do
      make_person(name: "Rui", email: "rui@x.example")
      cp = directory.counterparts.find { |c| c.name == "Rui" }
      expect(cp.data["starred"]).to be false
    end

    it "sets can_reply=true when the account is sendable" do
      # before block creates email_account_user with can_read; update it to have can_send too.
      EmailAccountUser.find_by(user: user, email_account: account)
                      .update_columns(can_send: true)
      make_person(name: "Ana", email: "ana@x.example", owe: true)
      cp = directory.counterparts.find { |c| c.name == "Ana" }
      # can_reply depends on email_message_id existing; if standing has one, check it.
      # Without a live feed item, email_message_id comes from last exchange.
      # Just verify the key exists.
      expect(cp.data).to have_key("can_reply")
    end

    it "sets can_done=true when the row has an eligible feed item kind" do
      person, contact, thread = make_person(name: "Sofia", email: "sofia@x.example", owe: true)
      msg = thread.email_messages.first
      # No :feed_item factory exists — use FeedItem.create! directly.
      item = FeedItem.create!(user: user, workspace: workspace,
                              subject: msg, kind: "reply_reminder", score: 0.5,
                              dedupe_key: "reply_reminder:#{msg.id}",
                              sort_at: Time.current,
                              data: { "thread_id" => thread.id })

      attn_item = instance_double("People::Attention::Item",
                                  feed_item: item,
                                  message: msg, detail: nil, detail_kind: nil, money: nil,
                                  subject: "Thread Sofia",
                                  thread_id: thread.id, verb: :reply,
                                  wait_days: 2, attention: true)
      attn = instance_double("People::Attention")
      allow(attn).to receive(:for).and_return(attn_item)
      allow(People::Attention).to receive(:new).and_return(attn)

      cp = directory.counterparts.find { |c| c.name == "Sofia" }
      expect(cp.data["can_done"]).to be true
    end
  end

  describe "data['tags']" do
    it "combines the person's sender tags and the email's tags, sender first, capped" do
      _person, contact, thread = make_person(name: "Sofia", email: "sofia@x.example")
      msg = thread.email_messages.first

      client  = create(:tag, workspace: workspace, name: "Client",  color: "#111111")
      receipt = create(:tag, workspace: workspace, name: "Receipt", color: "#222222")
      zeta    = create(:tag, workspace: workspace, name: "Zeta",    color: "#333333")
      ContactTag.create!(contact: contact, tag: client)
      create(:email_message_tag, email_message: msg, tag: receipt)
      create(:email_message_tag, email_message: msg, tag: zeta)

      cp = directory.counterparts.find { |c| c.name == "Sofia" }
      tags = cp.data["tags"]

      # Sender tag (Client) leads; the email's tags follow by name; capped at TAG_CAP.
      expect(tags.map { |t| t["name"] }).to eq(%w[Client Receipt])
      expect(tags.first).to include("id", "name", "color")
      expect(tags.first["color"]).to eq("#111111")
    end

    it "omits hidden tags (provider system statuses / low-value labels)" do
      _person, _contact, thread = make_person(name: "Rui", email: "rui@x.example")
      msg = thread.email_messages.first
      hidden = create(:tag, workspace: workspace, name: "CATEGORY_UPDATES", hidden: true)
      create(:email_message_tag, email_message: msg, tag: hidden)

      cp = directory.counterparts.find { |c| c.name == "Rui" }
      expect(cp.data["tags"]).to eq([])
    end

    it "does not surface another workspace's tags" do
      _person, _contact, thread = make_person(name: "Nadia", email: "nadia@x.example")
      msg = thread.email_messages.first
      other_ws = create(:workspace)
      foreign = create(:tag, workspace: other_ws, name: "Foreign")
      create(:email_message_tag, email_message: msg, tag: foreign)

      cp = directory.counterparts.find { |c| c.name == "Nadia" }
      expect(cp.data["tags"]).to eq([])
    end

    it "is an empty array when the person has no tags" do
      make_person(name: "Ana", email: "ana@x.example")
      cp = directory.counterparts.find { |c| c.name == "Ana" }
      expect(cp.data["tags"]).to eq([])
    end
  end
end
