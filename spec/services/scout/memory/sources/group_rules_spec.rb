# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::GroupRules do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with an InboxGroupRule for sender type" do
      let!(:rule) do
        InboxGroupRule.create!(workspace: ws, group_name: "Notifications", rule_type: "sender", value: "@github.com")
      end

      it "has the correct plain sentence" do
        entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
        expect(entry.plain).to eq("Mail from @github.com is a stream: Notifications.")
      end

      it "has :streams facet" do
        entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
        expect(entry.facet).to eq(:streams)
      end

      it "has :taught origin" do
        entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
        expect(entry.origin).to eq(:taught)
      end

      it "has the correct id" do
        entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
        expect(entry).not_to be_nil
      end

      it "has edit and remove actions" do
        entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
        expect(entry.actions).to eq(%i[edit remove])
      end
    end

    context "with a tag that has a default_bucket" do
      let!(:tag) do
        create(:tag, workspace: ws, name: "Newsletters & promos", default_bucket: "promotions")
      end

      it "produces a default bucket entry" do
        entry = source.entries.find { |e| e.id == "groupdefault:promotions" }
        expect(entry).not_to be_nil
      end

      it "has the correct plain sentence" do
        entry = source.entries.find { |e| e.id == "groupdefault:promotions" }
        expect(entry.plain).to eq("Newsletters & promos stays out of your priority stack.")
      end

      it "has :default origin" do
        entry = source.entries.find { |e| e.id == "groupdefault:promotions" }
        expect(entry.origin).to eq(:default)
      end

      it "has only edit action" do
        entry = source.entries.find { |e| e.id == "groupdefault:promotions" }
        expect(entry.actions).to eq(%i[edit])
      end
    end
  end

  describe "#remove" do
    let!(:rule) do
      InboxGroupRule.create!(workspace: ws, group_name: "Notifications", rule_type: "sender", value: "@github.com")
    end

    it "destroys the rule and returns true" do
      entry = source.entries.find { |e| e.id == "group:#{rule.id}" }
      expect { source.remove(entry) }.to change(InboxGroupRule, :count).by(-1)
    end
  end
end
