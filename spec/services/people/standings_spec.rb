# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Standings do
  around { |example| travel_to(Time.zone.local(2026, 9, 4, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Features).to receive(:bold_layout?).and_return(true)
  end

  def make_person(name:, email:, inbound_at: 2.days.ago, owe: false, source: "heuristic", emails: 1)
    person  = create(:person, workspace: workspace, name: name, context_summary: nil)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     name: name, email: email, sender_kind: :person, sender_kind_source: source)
    thread  = create(:email_thread, email_account: account, subject: "Thread #{name}")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
           from_address: email, subject: "Msg #{name}", received_at: inbound_at)
    contact.update_columns(email_count: emails, last_email_at: inbound_at)
    thread.update_columns(last_inbound_at: inbound_at) if owe
    [ person, contact, thread ]
  end

  describe ".refresh!" do
    it "writes one row per counterpart with correct score, kind, and name" do
      # Without a live feed item the standing falls back to :last_exchange.
      # needs_you comes from attention (feed items), which are empty here.
      person, = make_person(name: "Sofia", email: "sofia@x.example", inbound_at: 3.days.ago)

      described_class.refresh!(user)

      row = PeopleStanding.for_user(user).find_by!(counterpart: person)
      expect(row.needs_you).to be false
      expect(row.standing_kind).to be_in(PeopleStanding::STANDING_KINDS)
      expect(row.score).to be > 0
      expect(row.name).to eq("Sofia")
    end

    it "is idempotent: running twice yields the same row ids" do
      make_person(name: "Ana", email: "ana@x.example", inbound_at: 1.day.ago)

      described_class.refresh!(user)
      first_ids = PeopleStanding.for_user(user).pluck(:id).sort

      described_class.refresh!(user)
      second_ids = PeopleStanding.for_user(user).pluck(:id).sort

      expect(second_ids).to eq(first_ids)
      expect(PeopleStanding.for_user(user).count).to eq(1)
    end

    it "deletes a counterpart row when that person is no longer eligible" do
      person_a, = make_person(name: "Active", email: "active@x.example")
      person_b, = make_person(name: "Gone", email: "gone@x.example")

      described_class.refresh!(user)
      expect(PeopleStanding.for_user(user).count).to eq(2)

      # Remove person_b's contact email_count so it drops out
      person_b.contacts.update_all(email_count: 0)
      described_class.refresh!(user)

      expect(PeopleStanding.for_user(user).count).to eq(1)
      expect(PeopleStanding.for_user(user).first.name).to eq("Active")
    end

    it "scopes rows per user (each user's rows belong to that user)" do
      other_user = create(:user, workspace: workspace)
      create(:email_account_user, user: other_user, email_account: account, can_read: true)

      make_person(name: "Shared Person", email: "shared@x.example", owe: true, inbound_at: 2.days.ago)

      described_class.refresh!(user)
      described_class.refresh!(other_user)

      # Both users get rows, but they are distinct row records (different user_id).
      expect(PeopleStanding.for_user(user).count).to be > 0
      expect(PeopleStanding.for_user(other_user).count).to be > 0
      expect(PeopleStanding.where(user_id: user.id).pluck(:id))
        .not_to include(*PeopleStanding.where(user_id: other_user.id).pluck(:id))
    end

    it "writes the streams count to Rails.cache" do
      Rails.cache.with_local_cache do
        described_class.refresh!(user)
        cached = Rails.cache.read("people_streams_count_#{user.id}")
        expect(cached).not_to be_nil
      end
    end

    it "returns the number of counterpart rows" do
      make_person(name: "A", email: "a@x.example")
      make_person(name: "B", email: "b@x.example")
      count = described_class.refresh!(user)
      expect(count).to eq(2)
    end
  end

  describe ".missing?" do
    it "returns true when the user has no rows" do
      expect(described_class.missing?(user)).to be true
    end

    it "returns false after a refresh" do
      make_person(name: "Ana", email: "ana@x.example")
      described_class.refresh!(user)
      expect(described_class.missing?(user)).to be false
    end
  end

  describe ".refresh_counterpart!" do
    it "updates a single row when the person is still eligible" do
      person, contact, = make_person(name: "Sofia", email: "sofia@x.example")
      described_class.refresh!(user)

      row_before = PeopleStanding.for_user(user).find_by!(counterpart: person)

      # Change the contact name so the row name changes on re-compute.
      person.update!(name: "Sofia Updated")
      contact.update_columns(name: "Sofia Updated")

      described_class.refresh_counterpart!(user, person)

      row_after = PeopleStanding.for_user(user).find_by!(counterpart: person)
      expect(row_after.name).to eq("Sofia Updated")
    end

    it "deletes the row when the person is no longer eligible" do
      person, = make_person(name: "Sofia", email: "sofia@x.example")
      described_class.refresh!(user)
      expect(PeopleStanding.for_user(user).find_by(counterpart: person)).to be_present

      # Make person ineligible.
      person.contacts.update_all(email_count: 0)

      described_class.refresh_counterpart!(user, person)

      expect(PeopleStanding.for_user(user).find_by(counterpart: person)).to be_nil
    end

    it "broadcasts a replace when the row changed" do
      person, = make_person(name: "Sofia", email: "sofia@x.example")
      described_class.refresh!(user)
      # Force updated_at to age so we can detect a change.
      PeopleStanding.for_user(user).update_all(updated_at: 10.minutes.ago)

      # have_broadcasted_to requires ActionCable::TestHelper; use a message expectation instead.
      expect(Turbo::StreamsChannel).to receive(:broadcast_replace_to).with(
        "people_#{user.id}", hash_including(target: a_string_starting_with("people_row_"))
      ).at_least(:once)
      described_class.refresh_counterpart!(user, person)
    end

    it "does not raise when the person is nil" do
      expect { described_class.refresh_counterpart!(user, nil) }.not_to raise_error
    end
  end

  describe ".refresh! broadcasts" do
    it "broadcasts changed rows after a re-run that sees differences" do
      person_a, = make_person(name: "A", email: "a@x.example")
      _person_b, = make_person(name: "B", email: "b@x.example")
      described_class.refresh!(user)

      # Force both rows to look old so the next refresh detects changes.
      PeopleStanding.for_user(user).update_all(updated_at: 1.hour.ago)

      # have_broadcasted_to requires ActionCable::TestHelper; use a message expectation instead.
      expect(Turbo::StreamsChannel).to receive(:broadcast_replace_to).at_least(:once)
      described_class.refresh!(user)
    end
  end

  describe ".stale?" do
    it "returns true when there are no rows" do
      expect(described_class.stale?(user)).to be true
    end

    it "returns false right after a refresh" do
      make_person(name: "Ana", email: "ana@x.example")
      described_class.refresh!(user)
      expect(described_class.stale?(user, threshold: 10.minutes)).to be false
    end

    it "returns true after the threshold has passed" do
      make_person(name: "Ana", email: "ana@x.example")
      described_class.refresh!(user)
      # Back-date refreshed_at to simulate staleness.
      PeopleStanding.for_user(user).update_all(refreshed_at: 20.minutes.ago)
      expect(described_class.stale?(user, threshold: 10.minutes)).to be true
    end
  end
end
