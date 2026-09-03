# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Defaults do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    it "produces exactly one entry" do
      expect(source.entries.size).to eq(1)
    end

    it "has id 'default:review'" do
      expect(source.entries.first.id).to eq("default:review")
    end

    it "has :filing facet" do
      expect(source.entries.first.facet).to eq(:filing)
    end

    it "has :default origin" do
      expect(source.entries.first.origin).to eq(:default)
    end

    it "has no actions" do
      expect(source.entries.first.actions).to eq([])
    end

    it "has a plain sentence that mentions 'review'" do
      expect(source.entries.first.plain).to include("review")
    end

    it "has a present plain sentence" do
      expect(source.entries.first.plain).to be_present
    end
  end
end
