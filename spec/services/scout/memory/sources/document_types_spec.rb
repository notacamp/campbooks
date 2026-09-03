# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::DocumentTypes do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with a builtin document type" do
      let!(:doc_type) { create(:document_type, workspace: ws, name: "invoice") }

      it "has :default origin" do
        expect(source.entries.first.origin).to eq(:default)
      end

      it "has the correct plain sentence" do
        expect(source.entries.first.plain).to eq("Documents that look like Invoice go to Paper › Invoice.")
      end

      it "has only edit action" do
        expect(source.entries.first.actions).to eq(%i[edit])
      end

      it "has the correct id" do
        expect(source.entries.first.id).to eq("doctype:#{doc_type.id}")
      end
    end

    context "with a custom document type" do
      let!(:doc_type) { create(:document_type, workspace: ws, name: "Widget spec") }

      it "has :taught origin" do
        expect(source.entries.first.origin).to eq(:taught)
      end
    end

    context "with a document type that has auto_star enabled" do
      let!(:doc_type) { create(:document_type, workspace: ws, name: "invoice", auto_star: true) }

      it "includes ', and I star them.' at the end of the plain sentence" do
        expect(source.entries.first.plain).to end_with(", and I star them.")
      end
    end
  end
end
