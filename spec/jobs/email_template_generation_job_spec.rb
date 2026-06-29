require "rails_helper"

RSpec.describe EmailTemplateGenerationJob, type: :job do
  let(:ws) { create(:workspace) }
  let(:t) { create(:email_template, workspace: ws, description: "A welcome email") }
  let(:ok_r) do
    EmailTemplates::HtmlGenerator::Result.new(
      ok: true, subject: "Hello {{ name }}", body_html: "<p>Hi {{ name }}</p>",
      variables_schema: [ { "key" => "name" } ], ai_provenance: { "provider" => "anthropic" }, error: nil
    )
  end
  let(:fail_r) do
    EmailTemplates::HtmlGenerator::Result.new(
      ok: false, subject: nil, body_html: nil, variables_schema: nil, ai_provenance: {}, error: "e"
    )
  end

  before { allow(EmailTemplates::HtmlGenerator).to receive(:call).and_return(ok_r) }

  it "writes the generated content and marks completed on success" do
    described_class.perform_now(t.id)
    t.reload
    expect(t.ai_completed?).to be true
    expect(t.subject).to eq("Hello {{ name }}")
    expect(t.body_html).to eq("<p>Hi {{ name }}</p>")
  end

  it "marks failed on error" do
    allow(EmailTemplates::HtmlGenerator).to receive(:call).and_return(fail_r)
    described_class.perform_now(t.id)
    expect(t.reload.ai_failed?).to be true
  end
end
