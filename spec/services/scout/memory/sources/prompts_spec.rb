# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Prompts do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with no custom prompts" do
      it "produces one entry per catalog purpose" do
        expected_count = Ai::PromptCatalog.all.size
        expect(source.entries.size).to eq(expected_count)
      end

      it "every entry has :default origin" do
        source.entries.each do |entry|
          expect(entry.origin).to eq(:default), "Expected :default for #{entry.id}"
        end
      end

      it "every entry has :style facet" do
        source.entries.each do |entry|
          expect(entry.facet).to eq(:style)
        end
      end

      it "uses 'prompt:<purpose>' as id" do
        catalog_entry = Ai::PromptCatalog.find("email_analysis")
        entry = source.entries.find { |e| e.id == "prompt:email_analysis" }
        expect(entry).not_to be_nil
      end

      it "plain references the catalog label and built-in guidance" do
        catalog_entry = Ai::PromptCatalog.find("email_analysis")
        entry = source.entries.find { |e| e.id == "prompt:email_analysis" }
        expect(entry.plain).to eq("For #{catalog_entry.label}, Scout uses the built-in guidance.")
      end
    end

    context "with a custom prompt" do
      let!(:prompt) do
        ws.ai_prompts.create!(purpose: "email_analysis", instructions: "Focus on money. Ignore newsletters.")
      end

      it "uses 'aiprompt:<id>' as id for the custom entry" do
        entry = source.entries.find { |e| e.id == "aiprompt:#{prompt.id}" }
        expect(entry).not_to be_nil
      end

      it "has :taught origin" do
        entry = source.entries.find { |e| e.id == "aiprompt:#{prompt.id}" }
        expect(entry.origin).to eq(:taught)
      end

      it "plain starts with 'When reading'" do
        entry = source.entries.find { |e| e.id == "aiprompt:#{prompt.id}" }
        expect(entry.plain).to start_with("When reading")
      end

      it "plain includes the first sentence of the instructions" do
        entry = source.entries.find { |e| e.id == "aiprompt:#{prompt.id}" }
        expect(entry.plain).to include("Focus on money.")
      end

      it "leaves other purposes with default entries" do
        default_entry = source.entries.find { |e| e.id == "prompt:task_extraction" }
        expect(default_entry.origin).to eq(:default)
      end
    end
  end
end
