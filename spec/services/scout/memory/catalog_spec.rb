# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Catalog do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }

  let!(:rule) do
    ws.email_rules.create!(
      name: "EDP Rule",
      criteria: { "from" => ["@edp.pt"] },
      archive: true,
      created_by: user
    )
  end

  let!(:tag) do
    t = create(:tag, workspace: ws, name: "invoice")
    t.update!(prompt: "Invoices and receipts.")
    t
  end

  let!(:doc_type) { create(:document_type, workspace: ws, name: "Widget contract") }

  let!(:inbox_rule) do
    InboxGroupRule.create!(workspace: ws, group_name: "Notifications", rule_type: "sender", value: "@github.com")
  end

  let!(:starred_contact) do
    c = create(:contact, workspace: ws, name: "Sofia Martins", email: "sofia@x.com")
    c.update!(starred_at: Time.current)
    c
  end

  let(:catalog) { described_class.for(ws, user) }

  describe "#total and #entries" do
    it "total equals entries.size" do
      expect(catalog.total).to eq(catalog.entries.size)
    end

    it "total is positive" do
      expect(catalog.total).to be > 0
    end
  end

  describe "#facet_counts" do
    it "returns an array of [facet, count] pairs" do
      counts = catalog.facet_counts
      expect(counts).to be_an(Array)
      counts.each do |(facet, count)|
        expect(Scout::Memory::Entry::FACETS).to include(facet)
        expect(count).to be_positive
      end
    end

    it "includes :filing facet" do
      facets = catalog.facet_counts.map(&:first)
      expect(facets).to include(:filing)
    end

    it "includes :stack facet" do
      facets = catalog.facet_counts.map(&:first)
      expect(facets).to include(:stack)
    end

    it "includes :streams facet" do
      facets = catalog.facet_counts.map(&:first)
      expect(facets).to include(:streams)
    end

    it "only lists facets with positive counts" do
      catalog.facet_counts.each do |(_, count)|
        expect(count).to be_positive
      end
    end
  end

  describe "#entries_for" do
    it "returns only :streams entries for :streams facet" do
      streams = catalog.entries_for(:streams)
      expect(streams).not_to be_empty
      streams.each { |e| expect(e.facet).to eq(:streams) }
    end

    it "returns everything for :all" do
      expect(catalog.entries_for(:all).size).to eq(catalog.total)
    end

    it "returns everything for nil" do
      expect(catalog.entries_for(nil).size).to eq(catalog.total)
    end
  end

  describe "#entry" do
    it "returns the matching entry by id" do
      found = catalog.entry("rule:#{rule.id}")
      expect(found).not_to be_nil
      expect(found.id).to eq("rule:#{rule.id}")
    end

    it "returns nil for an unknown id" do
      expect(catalog.entry("rule:does-not-exist")).to be_nil
    end
  end

  describe "#perform" do
    it "removes the rule when action is :remove" do
      expect { catalog.perform(:remove, "rule:#{rule.id}") }
        .to change(EmailRule, :count).by(-1)
    end

    it "returns the entry when removing" do
      result = catalog.perform(:remove, "rule:#{rule.id}")
      expect(result).to be_a(Scout::Memory::Entry)
      expect(result.id).to eq("rule:#{rule.id}")
    end

    it "returns nil for an unknown id" do
      expect(catalog.perform(:remove, "nope")).to be_nil
    end

    it "returns nil when action is :confirm but entry has no :confirm action" do
      expect(catalog.perform(:confirm, "rule:#{rule.id}")).to be_nil
    end
  end

  describe "entry id stability" do
    it "yields the same id for the same rule across two catalog builds" do
      id1 = described_class.for(ws, user).entry("rule:#{rule.id}")&.id
      id2 = described_class.for(ws, user).entry("rule:#{rule.id}")&.id
      expect(id1).to eq(id2)
    end
  end
end
