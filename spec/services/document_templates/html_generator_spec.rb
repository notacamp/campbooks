require "rails_helper"

RSpec.describe DocumentTemplates::HtmlGenerator do
  let(:workspace) { create(:workspace) }
  let(:adapter) { double(:adapter) }
  let(:config) do
    { adapter: adapter, provider: "anthropic", model: "claude-sonnet-4-6",
      max_tokens: 4000, temperature: 0.3, system_prompt: nil }
  end

  describe ".call" do
    context "when AI is not configured" do
      before { allow(Ai::Configuration).to receive(:for).and_return(nil) }

      it "returns a failure result and never raises" do
        result = described_class.call(user_description: "An invoice", workspace: workspace)

        expect(result.ok).to be false
        expect(result.error).to be_present
        expect(result.html_content).to be_nil
      end
    end

    context "when the AI responds" do
      before do
        allow(Ai::Configuration).to receive(:for).with(:document_template_generation).and_return(config)
        allow(Ai::Provenance).to receive(:from_config).and_return({ "provider" => "anthropic" })
      end

      it "parses html_content and variables_schema" do
        allow(adapter).to receive(:chat).and_return(
          { html_content: "<h1>{{ name }}</h1>", variables_schema: [ { "key" => "name" } ] }.to_json
        )

        result = described_class.call(user_description: "Greeting", workspace: workspace)

        expect(result.ok).to be true
        expect(result.html_content).to include("{{ name }}")
        expect(result.variables_schema).to eq([ { "key" => "name" } ])
        expect(result.ai_provenance).to eq({ "provider" => "anthropic" })
      end

      it "fails when the AI returns no HTML" do
        allow(adapter).to receive(:chat).and_return({ html_content: "" }.to_json)

        expect(described_class.call(user_description: "Empty", workspace: workspace).ok).to be false
      end

      it "coerces a non-array schema to an empty array" do
        allow(adapter).to receive(:chat).and_return({ html_content: "<p>x</p>", variables_schema: "oops" }.to_json)

        expect(described_class.call(user_description: "x", workspace: workspace).variables_schema).to eq([])
      end
    end
  end
end
