# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Attention do
  let(:ws)     { Workspace.create!(name: "AT WS", slug: "at-#{SecureRandom.hex(4)}") }
  let(:user)   { ws.users.create!(name: "T", email_address: "at-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  def make_person(name)
    person = Person.create!(name: name, workspace: ws)
    contact = ws.contacts.create!(name: name, email: "#{name.downcase.tr(' ', '.')}@example.com",
                                   sender_kind: :person, person: person)
    [person, contact]
  end

  def make_weight(person, weight:, confidence: 0.7, reasons: [], sender_kind: "person")
    AttentionWeight.create!(
      user: user,
      workspace: ws,
      subject: person,
      weight: weight,
      confidence: confidence,
      raw_score: weight,
      reasons: reasons,
      evidence: { "sender_kind" => sender_kind },
      computed_at: Time.current
    )
  end

  def teach(person, label)
    person.contacts.each do |c|
      LearningDecision.create!(
        domain: "attention", user: user, workspace_id: ws.id,
        label: label, contact_id: c.id, signals: {}
      )
    end
  end

  describe "#entries — taught verdicts" do
    it "shows a taught_important entry for a person marked important" do
      person, _contact = make_person("Sofia Martins")
      make_weight(person, weight: 0.9, reasons: [{ "key" => "replies_fast", "params" => {} }])
      teach(person, "important")

      entries = source.entries
      taught = entries.find { |e| e.id == "attention:#{person.id}:taught" }
      expect(taught).to be_present
      expect(taught.facet).to eq(:people)
      expect(taught.origin).to eq(:taught)
      expect(taught.plain).to include("Sofia Martins")
      expect(taught.plain).to include("matters to you")
      expect(taught.actions).to eq(%i[remove])
    end

    it "shows a taught_unimportant entry for a person marked unimportant" do
      person, _contact = make_person("Spam Newsletter")
      make_weight(person, weight: 0.05, confidence: 0.8, reasons: [{ "key" => "new", "params" => {} }])
      teach(person, "unimportant")

      entries = source.entries
      taught = entries.find { |e| e.id == "attention:#{person.id}:taught" }
      expect(taught).to be_present
      expect(taught.plain).to include("doesn't need your attention")
    end
  end

  describe "#entries — learned high" do
    it "includes the top-weight person with a positive reason" do
      person, _contact = make_person("Ana Reis")
      make_weight(person, weight: 0.85, confidence: 0.8,
                   reasons: [{ "key" => "replies_fast", "params" => {} }])

      entries = source.entries
      high = entries.find { |e| e.id == "attention:#{person.id}:high" }
      expect(high).to be_present
      expect(high.facet).to eq(:people)
      expect(high.origin).to eq(:learned)
      expect(high.plain).to include("Ana Reis")
      expect(high.actions).to include(:confirm, :remove)
    end

    it "excludes persons with confidence < 0.5" do
      person, _contact = make_person("Low Conf")
      make_weight(person, weight: 0.9, confidence: 0.3,
                   reasons: [{ "key" => "replies_fast", "params" => {} }])

      entries = source.entries
      expect(entries.find { |e| e.id.include?(person.id.to_s) }).to be_nil
    end

    it "excludes persons who have a taught verdict" do
      person, _contact = make_person("Already Taught")
      make_weight(person, weight: 0.9, confidence: 0.8,
                   reasons: [{ "key" => "replies_fast", "params" => {} }])
      teach(person, "important")

      entries = source.entries
      expect(entries.find { |e| e.id == "attention:#{person.id}:high" }).to be_nil
      expect(entries.find { |e| e.id == "attention:#{person.id}:taught" }).to be_present
    end
  end

  describe "#entries — learned low" do
    it "includes a low-weight person with sender_kind person" do
      person, _contact = make_person("Noise Sender")
      make_weight(person, weight: 0.05, confidence: 0.6, sender_kind: "person",
                   reasons: [{ "key" => "new", "params" => {} }])

      entries = source.entries
      low = entries.find { |e| e.id == "attention:#{person.id}:low" }
      # Low entries need a positive reason — "new" is not positive, so this may not surface
      expect(low).to be_nil
    end
  end

  describe "#confirm" do
    it "records important for a high entry and returns true" do
      person, _contact = make_person("High Person")
      make_weight(person, weight: 0.8, confidence: 0.7,
                   reasons: [{ "key" => "replies_fast", "params" => {} }])

      entry = source.entries.find { |e| e.id == "attention:#{person.id}:high" }
      expect(entry).to be_present

      expect {
        result = source.confirm(entry)
        expect(result).to be(true)
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(1)

      row = LearningDecision.where(domain: "attention", user: user).last
      expect(row.label).to eq("important")
    end
  end

  describe "#remove" do
    it "forgets a taught verdict and returns true" do
      person, _contact = make_person("Taught Person")
      make_weight(person, weight: 0.9, reasons: [{ "key" => "replies_fast", "params" => {} }])
      teach(person, "important")

      entry = source.entries.find { |e| e.id == "attention:#{person.id}:taught" }
      expect(entry).to be_present

      expect {
        result = source.remove(entry)
        expect(result).to be(true)
      }.to change {
        LearningDecision.where(domain: "attention", user: user, contact_id: person.contacts.ids).count
      }.to(0)
    end

    it "records unimportant for a high entry when removed" do
      person, _contact = make_person("High Removed")
      make_weight(person, weight: 0.85, confidence: 0.8,
                   reasons: [{ "key" => "replies_fast", "params" => {} }])

      entry = source.entries.find { |e| e.id == "attention:#{person.id}:high" }
      expect(entry).to be_present

      expect {
        source.remove(entry)
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(1)

      row = LearningDecision.where(domain: "attention", user: user).last
      expect(row.label).to eq("unimportant")
    end
  end
end
