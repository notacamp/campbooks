# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Filtering do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with no contacts (default blacklist workspace)" do
      it "has exactly one entry (the strategy)" do
        expect(source.entries.size).to eq(1)
      end

      it "has the correct plain sentence for blacklist strategy" do
        expect(source.entries.first.plain).to eq("Blocked senders never enter the stack.")
      end

      it "has :default origin" do
        expect(source.entries.first.origin).to eq(:default)
      end

      it "has id 'filter:strategy'" do
        expect(source.entries.first.id).to eq("filter:strategy")
      end
    end

    context "with a whitelist workspace" do
      before { ws.update!(settings: ws.settings.merge("inbox_filter_strategy" => "whitelist")) }

      it "has the correct plain sentence for whitelist strategy" do
        strategy_entry = source.entries.find { |e| e.id == "filter:strategy" }
        expect(strategy_entry.plain).to eq("Only mail from people I've allowed enters the stack.")
      end

      it "has :taught origin" do
        strategy_entry = source.entries.find { |e| e.id == "filter:strategy" }
        expect(strategy_entry.origin).to eq(:taught)
      end
    end

    context "with a starred contact" do
      let!(:contact) do
        c = create(:contact, workspace: ws, name: "Sofia Martins", email: "sofia@x.com")
        c.update!(starred_at: Time.current)
        c
      end

      it "produces a starred entry with the correct plain" do
        entry = source.entries.find { |e| e.id == "starred:#{contact.id}" }
        expect(entry.plain).to eq("Mail from Sofia Martins is priority.")
      end

      it "has :taught origin for starred entry" do
        entry = source.entries.find { |e| e.id == "starred:#{contact.id}" }
        expect(entry.origin).to eq(:taught)
      end

      it "has edit and remove actions" do
        entry = source.entries.find { |e| e.id == "starred:#{contact.id}" }
        expect(entry.actions).to eq(%i[edit remove])
      end
    end

    context "with a blocked contact" do
      let!(:contact) do
        c = create(:contact, workspace: ws, email: "spammer@bad.com")
        c.block!
        c
      end

      it "produces a blocked entry with the contact's email as plain" do
        entry = source.entries.find { |e| e.id == "blocked:#{contact.id}" }
        expect(entry.plain).to eq("spammer@bad.com is blocked.")
      end
    end
  end

  describe "#remove" do
    context "with a starred contact" do
      let!(:contact) do
        c = create(:contact, workspace: ws, name: "Sofia Martins", email: "sofia@x.com")
        c.update!(starred_at: Time.current)
        c
      end

      it "un-stars the contact" do
        entry = source.entries.find { |e| e.id == "starred:#{contact.id}" }
        source.remove(entry)
        expect(contact.reload.starred_at).to be_nil
      end
    end

    context "with a blocked contact" do
      let!(:contact) do
        c = create(:contact, workspace: ws, email: "blocked@bad.com")
        c.block!
        c
      end

      it "un-blocks the contact" do
        entry = source.entries.find { |e| e.id == "blocked:#{contact.id}" }
        source.remove(entry)
        expect(contact.reload.list_status).to eq("neutral")
      end
    end
  end
end
