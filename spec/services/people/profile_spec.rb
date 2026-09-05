# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Profile do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  # Shared thread for accessible messages.
  let(:thread)  { create(:email_thread, email_account: account) }

  let(:person)  { create(:person, workspace: workspace, name: "Tina Tester") }
  let(:contact) do
    create(:contact, workspace: workspace, email_account: account,
           person: person, email: "tina@example.com",
           sender_kind: :person, email_count: 5)
  end

  def make_message(contact:, outbound: false, received_at: 1.day.ago)
    # sent? is derived from from_address matching the account email address.
    account_email = account.email_address.to_s
    create(:email_message, email_account: account,
           contact: contact, email_thread: thread, received_at: received_at,
           from_address: outbound ? account_email : contact.email,
           to_address: outbound ? contact.email : account_email)
  end

  describe ".for" do
    subject(:profile) { described_class.for(person, user: user) }

    context "with no contacts" do
      let(:lonely_person) { create(:person, workspace: workspace, name: "Lone Ranger") }

      subject(:profile) { described_class.for(lonely_person, user: user) }

      it "returns empty counts" do
        expect(profile.counts[:received]).to eq(0)
        expect(profile.threads).to be_empty
        expect(profile.documents).to be_empty
        expect(profile.events).to be_empty
      end
    end

    context "with a contact and messages" do
      before do
        contact  # instantiate
        make_message(contact: contact, received_at: 2.days.ago)
        make_message(contact: contact, outbound: true, received_at: 1.day.ago)
      end

      it "returns the primary contact (highest email_count)" do
        expect(profile.primary_contact).to eq(contact)
      end

      it "includes the contact email in emails" do
        addresses = profile.emails.map(&:first)
        expect(addresses).to include("tina@example.com")
      end

      it "marks the primary contact email as primary" do
        primary = profile.emails.find { |_a, p| p }
        expect(primary).not_to be_nil
        expect(primary.first).to eq("tina@example.com")
      end

      it "counts received messages" do
        expect(profile.counts[:received]).to be >= 1
      end

      it "counts sent messages" do
        expect(profile.counts[:sent]).to be >= 1
      end

      it "counts threads" do
        expect(profile.counts[:threads]).to be >= 1
      end

      it "counts your own replies in the person's threads even when they carry no contact" do
        create(:email_message, email_account: account, contact: nil, email_thread: thread,
               received_at: 12.hours.ago,
               from_address: "Me <#{account.email_address}>", to_address: contact.email)

        expect(profile.counts[:sent]).to eq(2)
        expect(profile.counts[:received]).to eq(1)
        expect(profile.threads.first[:count]).to eq(3) # thread-wide, not contact-linked only
      end
    end

    context "email aliases" do
      let(:alias_contact) do
        create(:contact, workspace: workspace, email_account: account,
               person: person, email: "alias@example.com", email_count: 1)
      end

      before do
        contact
        alias_contact
        # Add an email alias to the primary contact
        create(:contact_email_alias, contact: contact, email: "tina.work@example.com")
      end

      it "includes aliases in emails without duplicates" do
        addresses = profile.emails.map(&:first)
        expect(addresses).to include("tina@example.com")
        expect(addresses).to include("tina.work@example.com")
        expect(addresses).to include("alias@example.com")
        expect(addresses.uniq.size).to eq(addresses.size)
      end
    end

    context "documents via document_email_messages" do
      let(:message) { make_message(contact: contact) }
      let(:doc) { create(:document, workspace: workspace) }

      before do
        contact
        message
        DocumentEmailMessage.create!(document: doc, email_message: message)
      end

      it "includes the document" do
        expect(profile.documents.map(&:id)).to include(doc.id)
      end
    end

    context "events via attendee email" do
      let(:calendar_account) { create(:calendar_account, workspace: workspace) }
      let(:calendar) { create(:calendar, calendar_account: calendar_account) }

      before do
        contact
        create(:calendar_account_user, user: user, calendar_account: calendar_account, can_read: true)
      end

      it "includes events where the person is an attendee" do
        ev = create(:calendar_event, calendar: calendar,
                    attendees: [ { "email" => "tina@example.com", "rsvp_status" => "accepted" } ],
                    start_at: 1.day.from_now, end_at: 1.day.from_now + 1.hour)
        expect(profile.events.map(&:id)).to include(ev.id)
      end
    end

    context "events via source email message" do
      let(:calendar_account) { create(:calendar_account, workspace: workspace) }
      let(:calendar) { create(:calendar, calendar_account: calendar_account) }

      before do
        create(:calendar_account_user, user: user, calendar_account: calendar_account, can_read: true)
      end

      it "includes events sourced from the person's messages" do
        contact
        msg = make_message(contact: contact)
        ev  = create(:calendar_event, calendar: calendar,
                     source_email_message: msg,
                     start_at: 1.day.from_now, end_at: 1.day.from_now + 1.hour)
        expect(profile.events.map(&:id)).to include(ev.id)
      end
    end

    context "list_status rollup" do
      before { contact }

      it "is :blocked when any contact is blocked" do
        contact.update!(list_status: :blocked)
        expect(profile.list_status).to eq(:blocked)
      end

      it "is :allowed when any contact is allowed (and none blocked)" do
        contact.update!(list_status: :allowed)
        expect(profile.list_status).to eq(:allowed)
      end

      it "is :neutral when all contacts are neutral" do
        expect(profile.list_status).to eq(:neutral)
      end
    end

    context "starred? rollup" do
      before { contact }

      it "is true when any contact is starred" do
        contact.update!(starred_at: Time.current)
        expect(profile.starred?).to be true
      end

      it "is false when no contact is starred" do
        expect(profile.starred?).to be false
      end
    end

    context "attention weight" do
      before { contact }

      it "exposes the person's attention weight row when one exists" do
        row = AttentionWeight.create!(user: user, workspace: workspace, subject: person,
                                      weight: 0.8, confidence: 0.9,
                                      reasons: [ { "key" => "two_way", "params" => { "count" => 3 } } ],
                                      computed_at: Time.current)
        expect(profile.attention).to eq(row)
      end

      it "is nil when the person has no attention row" do
        expect(profile.attention).to be_nil
      end
    end

    context "query budget" do
      before do
        contact
        make_message(contact: contact)
      end

      it "resolves in ≤ 14 queries" do
        # ≤ 14: two queries for build_events (upcoming + past) plus one for the
        # learned attention weight (Attention::Weights#for). Warm all caches first.
        described_class.for(person, user: user)

        query_count = 0
        counter = ->(*, **) { query_count += 1 }

        ActiveSupport::Notifications.subscribed(counter, "sql.active_record") do
          described_class.for(person, user: user)
        end

        expect(query_count).to be <= 14
      end
    end
  end
end
