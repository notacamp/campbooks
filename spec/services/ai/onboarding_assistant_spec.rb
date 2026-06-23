require "rails_helper"

RSpec.describe Ai::OnboardingAssistant do
  let(:workspace) { create(:workspace) }
  subject(:assistant) { described_class.new(workspace) }

  # Stub the resolved AI config so no real provider is called. Returns the
  # adapter double so individual examples can assert on what it received.
  def stub_adapter(reply)
    adapter = instance_double(Ai::Adapters::Openai)
    allow(adapter).to receive(:chat).and_return(reply)
    allow(Ai::Configuration).to receive(:for).with("global_chat")
      .and_return(adapter: adapter, model: "m", max_tokens: 100, temperature: 0.3)
    adapter
  end

  describe "#conversational_turn" do
    it "returns a question turn with its hint" do
      stub_adapter('{"question":"What do you do?","hint":"e.g. a law firm"}')
      result = assistant.conversational_turn(history: [], kind: :document_types)
      expect(result).to include(type: :question, question: "What do you do?", hint: "e.g. a law firm")
    end

    it "returns a normalized proposal turn (snake_case name, schema kept)" do
      stub_adapter('{"proposal":{"document_types":[{"name":"Client Invoice","color":"#3b82f6","prompt":"x","extraction_schema":{"total":{"type":"number"}}}]}}')
      result = assistant.conversational_turn(history: [ { role: "user", content: "law firm" } ], kind: :document_types)
      expect(result[:type]).to eq(:proposal)
      expect(result[:items].first["name"]).to eq("client_invoice")
      expect(result[:items].first["extraction_schema"]).to be_present
    end

    it "tolerates markdown fences and a bare array proposal (tags)" do
      stub_adapter("```json\n[{\"name\":\"urgent\",\"color\":\"#ef4444\",\"prompt\":\"x\"}]\n```")
      result = assistant.conversational_turn(history: [], kind: :tags)
      expect(result[:type]).to eq(:proposal)
      expect(result[:items].first["name"]).to eq("urgent")
      expect(result[:items].first).not_to have_key("extraction_schema")
    end

    it "returns an error turn on unparseable output" do
      stub_adapter("sorry, I can't help with that")
      result = assistant.conversational_turn(history: [], kind: :tags)
      expect(result[:type]).to eq(:error)
    end

    it "returns no_ai_config when no provider is configured" do
      allow(Ai::Configuration).to receive(:for).and_return(nil)
      allow(Rails.application.config).to receive(:self_hosted).and_return(false)
      expect(assistant.conversational_turn(history: [], kind: :tags)).to eq(type: :error, reason: :no_ai_config)
    end

    it "forces a proposal once the question cap is reached" do
      captured = nil
      adapter = instance_double(Ai::Adapters::Openai)
      allow(adapter).to receive(:chat) { |**kw| captured = kw[:system]; '{"proposal":{"tags":[]}}' }
      allow(Ai::Configuration).to receive(:for).and_return(adapter: adapter, model: "m", max_tokens: 1, temperature: 0)
      history = Array.new(Ai::OnboardingAssistant::MAX_QUESTIONS) { { role: "assistant", content: "q" } }
      assistant.conversational_turn(history: history, kind: :tags)
      expect(captured).to include("MUST emit the proposal now")
    end
  end

  describe ".persist_proposal" do
    it "creates document types idempotently and keeps the schema" do
      items = [ { "name" => "invoice", "color" => "#fff", "prompt" => "x", "extraction_schema" => { "n" => 1 } } ]
      expect { described_class.persist_proposal(workspace: workspace, kind: :document_types, items: items) }
        .to change { workspace.document_types.count }.by(1)
      expect { described_class.persist_proposal(workspace: workspace, kind: :document_types, items: items) }
        .not_to change { workspace.document_types.count }
      expect(workspace.document_types.find_by(name: "invoice").extraction_schema).to eq("n" => 1)
    end

    it "creates local tags" do
      items = [ { "name" => "urgent", "color" => "#ef4444", "prompt" => "x" } ]
      expect { described_class.persist_proposal(workspace: workspace, kind: :tags, items: items) }
        .to change { workspace.tags.count }.by(1)
      expect(workspace.tags.find_by(name: "urgent").source).to eq("local")
    end
  end
end
