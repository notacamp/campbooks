# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Directory do
  around { |example| travel_to(Time.zone.local(2026, 9, 4, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Features).to receive(:bold_layout?).and_return(true)
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

    it "includes an org when it has a person member with mail" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      person, = make_person(name: "Rui", email: "rui@cloudhost.example")
      create(:organization_membership, person: person, organization: org)

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

    it "ranks an org as its lead person" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      person, = make_person(name: "Rui Santos", email: "rui@cloudhost.example", owe: true,
                            inbound_at: 2.days.ago, emails: 5)
      create(:organization_membership, person: person, organization: org)

      counterparts = directory.counterparts
      rui      = counterparts.find { |c| c.name == "Rui Santos" }
      cloudhost = counterparts.find { |c| c.name == "Cloudhost" }

      # Org should have the same score as its lead person, so it sits right behind
      expect(cloudhost.priority).to be > 0
      expect(rui.priority).to be >= cloudhost.priority
    end
  end
end
