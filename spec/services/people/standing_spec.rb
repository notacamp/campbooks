# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Standing do
  around { |example| travel_to(Time.zone.local(2026, 9, 3, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  # Builds a person + a thread. `last_inbound`/`last_outbound` set the thread's
  # denormalized reply state; a message is created for each present timestamp.
  def person_with(name:, context_summary: nil, ai_action_prompt: nil, last_inbound: nil, last_outbound: nil, last_email_at: nil)
    person = create(:person, workspace: workspace, name: name, context_summary: context_summary)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     email: "#{name.parameterize}@x.example", sender_kind: :person, email_count: 1,
                     last_email_at: last_email_at || last_inbound || 1.day.ago)
    thread = create(:email_thread, email_account: account, subject: "Q3 deck",
                    last_inbound_at: last_inbound, last_outbound_at: last_outbound)
    if last_inbound
      create(:email_message, email_account: account, email_thread: thread, contact: contact,
             from_address: contact.email, received_at: last_inbound, subject: "Q3 deck",
             ai_action_prompt: ai_action_prompt, body: "hello")
    end
    if last_outbound
      create(:email_message, email_account: account, email_thread: thread, from_address: account.email_address,
             received_at: last_outbound, subject: "Re: Q3 deck", body: "on it")
    end
    person
  end

  it "you owe a reply → needs you, pluralized" do
    owe2 = person_with(name: "Sofia", last_inbound: 2.days.ago, last_outbound: nil)
    st = described_class.for_person(owe2, user: user)
    expect(st.needs_you).to be true
    expect(st.text).to eq("Waiting on your reply for 2 days.")
    expect(st.overdue_days).to eq(2)

    owe1 = person_with(name: "Mara", last_inbound: 25.hours.ago, last_outbound: nil)
    expect(described_class.for_person(owe1, user: user).text).to eq("Waiting on your reply for a day.")
  end

  it "they owe you (nudge due) → No reply … Nudge?" do
    p = person_with(name: "Miguel", last_inbound: 12.days.ago, last_outbound: 6.days.ago)
    st = described_class.for_person(p, user: user)
    expect(st.needs_you).to be true
    expect(st.text).to include("No reply to").and include("Nudge?")
  end

  it "falls to the Scout action prompt when nothing is owed" do
    p = person_with(name: "Ana", last_inbound: 5.days.ago, last_outbound: 1.day.ago,
                    ai_action_prompt: "She needs one receipt from June.")
    st = described_class.for_person(p, user: user)
    expect(st.needs_you).to be false
    expect(st.text).to eq("She needs one receipt from June.")
  end

  it "falls to the profile summary's first sentence" do
    p = person_with(name: "David", last_inbound: 5.days.ago, last_outbound: 1.day.ago,
                    context_summary: "Long-time client. Prefers phone.")
    expect(described_class.for_person(p, user: user).text).to eq("Long-time client.")
  end

  it "falls back to the last exchange when there is nothing else" do
    p = person_with(name: "Quiet", context_summary: nil, last_email_at: 3.days.ago)
    st = described_class.for_person(p, user: user)
    expect(st.needs_you).to be false
    expect(st.text).to start_with("Last exchange")
  end

  it "composes an organization from its people's standings" do
    org = create(:organization, workspace: workspace, name: "Cloudhost")
    owe = person_with(name: "Rui", last_inbound: 3.days.ago, last_outbound: nil)
    create(:organization_membership, person: owe, organization: org)

    st = described_class.for_organization(org, user: user)
    expect(st.needs_you).to be true
    expect(st.text).to include("Waiting on your reply")
  end

  # The batched list path (prime + reuse one instance) must return byte-for-byte
  # the same Result as the single-record path (for_person/for_organization). This
  # guards against drift between person_threads / latest_inbound_message /
  # profile_summary / organization sampling and their primed equivalents.
  describe "priming parity (batched list path == single-record path)" do
    def primed(person)
      described_class.new(user, now: Time.current).prime(people: [ person ]).person(person)
    end

    it "matches the single-record path for every person priority" do
      cases = {
        you_owe:       person_with(name: "Sofia", last_inbound: 2.days.ago),
        nudge:         person_with(name: "Miguel", last_inbound: 12.days.ago, last_outbound: 6.days.ago),
        action_prompt: person_with(name: "Ana", last_inbound: 5.days.ago, last_outbound: 1.day.ago, ai_action_prompt: "She needs one receipt."),
        own_profile:   person_with(name: "David", last_inbound: 5.days.ago, last_outbound: 1.day.ago, context_summary: "Long-time client. Prefers phone."),
        last_exchange: person_with(name: "Quiet", last_email_at: 3.days.ago),
        nothing:       create(:person, workspace: workspace, name: "Empty", context_summary: nil)
      }

      cases.each do |label, person|
        expect(primed(person)).to eq(described_class.for_person(person, user: user)),
          "primed result diverged from live for #{label}"
      end
    end

    it "matches when the profile summary comes from a contact (not the person)" do
      person = create(:person, workspace: workspace, name: "Nadia", context_summary: nil)
      create(:contact, workspace: workspace, email_account: account, person: person,
             email: "nadia@x.example", sender_kind: :person, email_count: 4,
             last_email_at: 2.days.ago, context_summary: "Founder at Acme. Two open invoices.")

      expect(primed(person)).to eq(described_class.for_person(person, user: user))
    end

    it "matches for an organization" do
      org = create(:organization, workspace: workspace, name: "Cloudhost")
      owe = person_with(name: "Rui", last_inbound: 3.days.ago)
      create(:organization_membership, person: owe, organization: org)

      live = described_class.for_organization(org, user: user)
      primed = described_class.new(user, now: Time.current).prime(organizations: [ org ]).organization(org)
      expect(primed).to eq(live)
    end
  end
end
