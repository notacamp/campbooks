require "rails_helper"

RSpec.describe Ai::DocumentAnalyzer do
  # Regression: when the model proposes a NEW document type, apply_result used to
  # call `t.organization = ...`, but DocumentType has no `organization` (it
  # `belongs_to :workspace`). Every such document crashed with
  # NoMethodError and was recorded as failed. The fix scopes creation to the
  # document's workspace and drops the bogus setter.
  describe "#call when the model proposes a new document type" do
    let(:workspace) { create(:workspace) }
    let(:document)  { create(:document, workspace: workspace, document_type: :other, ai_status: :pending) }
    let(:adapter)   { instance_double(Ai::Adapters::Openai) }

    let(:ai_response) do
      {
        document_type: "vehicle_inspection_report", # not an existing type → must be created
        title: "Inspeção IPO — 2026",
        description: "Relatório de inspeção periódica obrigatória.",
        confidence: 0.95,
        suggested_filename: "inspecao_ipo_2026",
        metadata: {}
      }.to_json
    end

    before do
      Current.workspace = workspace
      allow(Ai::Configuration).to receive(:for).with("document_analysis").and_return(
        { adapter: adapter, model: "gpt-4o-mini", max_tokens: 1000, temperature: 0.0 }
      )
      allow(adapter).to receive(:chat).and_return(ai_response)
    end

    after { Current.workspace = nil }

    it "creates the DocumentType scoped to the document's workspace without raising" do
      expect {
        described_class.new(document).call
      }.to change { workspace.document_types.where(name: "vehicle_inspection_report").count }.by(1)

      document.reload
      expect(document.ai_status).to eq("completed")
      expect(document.ai_error).to be_nil
    end

    it "does not leak the type into a different workspace" do
      other_ws = create(:workspace)
      described_class.new(document).call
      expect(other_ws.document_types.where(name: "vehicle_inspection_report")).to be_empty
    end
  end
end
