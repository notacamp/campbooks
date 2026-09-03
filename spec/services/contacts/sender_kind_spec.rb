# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contacts::SenderKind do
  # Duck-typed messages: the categorizer + the People screen read from_address,
  # subject, category, and the bulk/automated headers.
  msg = Struct.new(:from_address, :subject, :category, :header_list_unsubscribe,
                   :header_precedence, :header_auto_submitted, keyword_init: true)

  def human(**over) = @msg.new(from_address: "sofia@brightloop.example", subject: "Re: the deck", category: "personal", **over)
  def bulk(**over)  = @msg.new(from_address: "news@shop.example", subject: "50% off!", category: "promotions", header_list_unsubscribe: "<mailto:u@x>", **over)

  before { @msg = msg }

  describe ".service?" do
    it "is false for a human majority" do
      expect(described_class.service?(Array.new(3) { human })).to be false
    end

    it "is true when the majority is machine/bulk" do
      expect(described_class.service?(Array.new(3) { bulk })).to be true
    end

    it "flags machine senders, bulk headers and machine categories" do
      expect(described_class.service_message?(msg.new(from_address: "no-reply@x.example", subject: "hi", category: "personal"))).to be true
      expect(described_class.service_message?(msg.new(from_address: "a@x.example", subject: "hi", category: "personal", header_precedence: "bulk"))).to be true
      expect(described_class.service_message?(msg.new(from_address: "a@x.example", subject: "hi", category: "notifications"))).to be true
      expect(described_class.service_message?(human)).to be false
    end

    it "uses the majority, not any single machine message" do
      mixed = [ human, human, bulk ] # 1 of 3 service → not a service
      expect(described_class.service?(mixed)).to be false
    end
  end

  describe ".classify" do
    let(:workspace) { create(:workspace) }
    let(:account) { create(:email_account, workspace: workspace) }

    def contact_with(from:, category:, list_unsub: nil, count: 3)
      contact = create(:contact, workspace: workspace, email_account: account, email: from,
                       person: create(:person, workspace: workspace), email_count: count)
      count.times do
        create(:email_message, email_account: account, contact: contact, from_address: from,
               category: category, header_list_unsubscribe: list_unsub)
      end
      contact
    end

    it "persists a service verdict, source and stream kind" do
      contact = contact_with(from: "news@shop.example", category: "promotions", list_unsub: "<mailto:u@x>")
      expect(described_class.classify(contact)).to eq(:service)
      contact.reload
      expect(contact.sender_kind).to eq("service")
      expect(contact.sender_kind_source).to eq("heuristic")
      expect(contact.stream_kind).to eq("newsletters")
    end

    it "persists a person verdict" do
      contact = contact_with(from: "sofia@brightloop.example", category: "personal")
      expect(described_class.classify(contact)).to eq(:person)
      expect(contact.reload.sender_kind).to eq("person")
      expect(contact.stream_kind).to be_nil
    end

    it "never overrides a taught verdict" do
      contact = contact_with(from: "news@shop.example", category: "promotions", list_unsub: "<mailto:u@x>")
      contact.update!(sender_kind: :person, sender_kind_source: "taught")
      expect(described_class.classify(contact)).to be_nil
      expect(contact.reload.sender_kind).to eq("person")
    end

    it "no-ops a contact with no mail" do
      contact = create(:contact, workspace: workspace, email_account: account, person: create(:person, workspace: workspace))
      expect(described_class.classify(contact)).to be_nil
    end
  end
end
