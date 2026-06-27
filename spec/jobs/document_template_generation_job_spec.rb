require "rails_helper"
RSpec.describe DocumentTemplateGenerationJob, type: :job do
  let(:ws) { create(:workspace) }
  let(:t) { create(:document_template, workspace: ws, description:"d") }
  let(:ok_r) { DocumentTemplates::HtmlGenerator::Result.new(ok:true, html_content:"<h>OK</h>", variables_schema:[], ai_provenance:{}, error:nil) }
  let(:fail_r) { DocumentTemplates::HtmlGenerator::Result.new(ok:false, html_content:nil, variables_schema:nil, ai_provenance:{}, error:"e") }
  before { allow(DocumentTemplates::HtmlGenerator).to receive(:call).and_return(ok_r) }
  it "sets completed on success" do
    described_class.perform_now(t.id)
    expect(t.reload.ai_completed?).to be true
  end
  it "sets failed on error" do
    allow(DocumentTemplates::HtmlGenerator).to receive(:call).and_return(fail_r)
    described_class.perform_now(t.id)
    expect(t.reload.ai_failed?).to be true
  end
end
