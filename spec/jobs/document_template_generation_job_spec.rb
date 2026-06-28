require "rails_helper"

RSpec.describe DocumentTemplateGenerationJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:template) { create(:document_template, workspace: workspace, description: "An invoice") }

  let(:success) do
    DocumentTemplates::HtmlGenerator::Result.success(
      html_content: "<h1>OK</h1>", variables_schema: [], ai_provenance: { "provider" => "anthropic" }
    )
  end
  let(:failure) { DocumentTemplates::HtmlGenerator::Result.failure("boom") }

  it "stores the generated HTML and marks the template completed" do
    allow(DocumentTemplates::HtmlGenerator).to receive(:call).and_return(success)

    described_class.perform_now(template.id)

    expect(template.reload).to have_attributes(ai_status: "completed", html_content: "<h1>OK</h1>")
  end

  it "marks the template failed when generation fails" do
    allow(DocumentTemplates::HtmlGenerator).to receive(:call).and_return(failure)

    described_class.perform_now(template.id)

    expect(template.reload.ai_failed?).to be true
  end
end
