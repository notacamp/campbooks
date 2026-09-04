# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::Standing do
  around { |example| travel_to(Time.zone.local(2026, 9, 3, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def person_with(name:, context_summary: nil, last_inbound: nil, last_outbound: nil, last_email_at: nil)
    person = create(:person, workspace: workspace, name: name, context_summary: context_summary)
    create(:contact, workspace: workspace, email_account: account, person: person,
           email: "#{name.parameterize}@x.example", sender_kind: :person, email_count: 1,
           last_email_at: last_email_at || last_inbound || 1.day.ago)
    if last_inbound
      thread = create(:email_thread, email_account: account, subject: "Hello",
                      last_inbound_at: last_inbound, last_outbound_at: last_outbound)
      create(:email_message, email_account: account, email_thread: thread, contact:
             Contact.find_by(person: person),
             from_address: "#{name.parameterize}@x.example", received_at: last_inbound,
             subject: "Hello", body: "hi")
    end
    person
  end

  # Builds a People::Attention double that returns `item` for any counterpart.
  def stub_attention_with(item, counterpart: nil)
    attn = instance_double(People::Attention)
    if counterpart
      allow(attn).to receive(:for).with(counterpart).and_return(item)
      allow(attn).to receive(:for).with(anything).and_return(nil)
    else
      allow(attn).to receive(:for).and_return(item)
    end
    attn
  end

  def stub_attention_item(verb:, subject: "Q3 deck", wait_days: 2, score: 50.0)
    fi = instance_double(FeedItem,
                         id: SecureRandom.uuid,
                         score: score,
                         sort_at: Time.current,
                         data: { "age_days" => wait_days })
    instance_double(People::Attention::Item,
                    feed_item: fi,
                    verb: verb,
                    subject: subject,
                    wait_days: wait_days,
                    text: nil,
                    thread_id: nil,
                    message: nil,
                    attention: true)
  end

  # ── Attention path ─────────────────────────────────────────────────────────

  it "with an attention item → needs_you true, kind :attention" do
    p = person_with(name: "Sofia", last_inbound: 2.days.ago)
    item = stub_attention_item(verb: :reply, subject: "Q3 deck", wait_days: 2)
    attn = stub_attention_with(item)

    st = described_class.for_person(p, user: user, attention: attn)
    expect(st.needs_you).to be true
    expect(st.kind).to eq(:attention)
    expect(st.verb).to eq(:reply)
    expect(st.subject).to eq("Q3 deck")
    expect(st.wait_days).to eq(2)
  end

  it "verb :nudge from a follow_up attention item" do
    p = person_with(name: "Miguel", last_inbound: 12.days.ago, last_outbound: 6.days.ago)
    item = stub_attention_item(verb: :nudge, subject: "Proposal", wait_days: 6)
    st = described_class.for_person(p, user: user, attention: stub_attention_with(item))
    expect(st.verb).to eq(:nudge)
    expect(st.needs_you).to be true
  end

  it "verb :decide from an email_action attention item" do
    p = person_with(name: "Ana", last_inbound: 3.days.ago)
    item = stub_attention_item(verb: :decide, subject: "Contract review", wait_days: 3)
    st = described_class.for_person(p, user: user, attention: stub_attention_with(item))
    expect(st.verb).to eq(:decide)
  end

  # ── Fallback path (no attention item) ─────────────────────────────────────

  it "falls to the profile summary's first sentence when no attention item" do
    p = person_with(name: "David", last_inbound: 5.days.ago, last_outbound: 1.day.ago,
                    context_summary: "Long-time client. Prefers phone.")
    st = described_class.for_person(p, user: user)
    expect(st.needs_you).to be false
    expect(st.kind).to eq(:summary)
    expect(st.text).to eq("Long-time client.")
  end

  it "falls back to last_exchange when no attention item and no summary" do
    p = person_with(name: "Quiet", context_summary: nil, last_email_at: 3.days.ago)
    st = described_class.for_person(p, user: user)
    expect(st.needs_you).to be false
    expect(st.kind).to eq(:last_exchange)
  end

  it "Result.none has kind :none and needs_you false" do
    expect(described_class::Result.none.kind).to eq(:none)
    expect(described_class::Result.none.needs_you).to be false
  end

  # ── Organization path ─────────────────────────────────────────────────────

  it "organization returns Result.none when no attention item" do
    org = create(:organization, workspace: workspace, name: "Acme")
    st = described_class.for_organization(org, user: user)
    expect(st.kind).to eq(:none)
    expect(st.needs_you).to be false
  end

  it "organization with attention item → needs_you true, kind :attention" do
    org = create(:organization, workspace: workspace, name: "Acme")
    item = stub_attention_item(verb: :chase, subject: "Invoice #42", wait_days: 14)
    st = described_class.for_organization(org, user: user, attention: stub_attention_with(item))
    expect(st.needs_you).to be true
    expect(st.kind).to eq(:attention)
    expect(st.verb).to eq(:chase)
  end

  # ── Priming parity ─────────────────────────────────────────────────────────
  #
  # The batched list path (prime + reuse one instance) must return byte-for-byte
  # the same Result as the single-record path (for_person/for_organization).

  describe "priming parity (batched list path == single-record path)" do
    def primed(person)
      described_class.new(user, now: Time.current).prime(people: [ person ]).person(person)
    end

    it "matches the single-record path for every fallback case" do
      cases = {
        own_profile:   person_with(name: "David", last_inbound: 5.days.ago, last_outbound: 1.day.ago,
                                   context_summary: "Long-time client. Prefers phone."),
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
      primed_result = described_class.new(user, now: Time.current).prime(organizations: [ org ]).organization(org)
      expect(primed_result).to eq(live)
    end
  end
end
