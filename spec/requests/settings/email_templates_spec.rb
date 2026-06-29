require "rails_helper"

RSpec.describe "Settings::EmailTemplates", type: :request do
  # A plan that includes email templates (create/regenerate are entitlement-gated).
  let(:ws) { create(:workspace, plan: "pro") }
  let(:u) { create(:user, workspace: ws) }

  before { allow(Features).to receive(:email_templates?).and_return(true) }

  it "requires auth" do
    get settings_email_templates_path
    expect(response).to redirect_to(new_session_path)
  end

  it "returns ok" do
    sign_in(u)
    get settings_email_templates_path
    expect(response).to have_http_status(:ok)
  end

  it "404 when disabled" do
    allow(Features).to receive(:email_templates?).and_return(false)
    sign_in(u)
    get settings_email_templates_path
    expect(response).to have_http_status(:not_found)
  end

  it "creates a template" do
    sign_in(u)
    expect do
      post settings_email_templates_path, params: { email_template: { name: "Welcome", subject: "Hi", body_html: "<p>Hi</p>" } }
    end.to change(EmailTemplate, :count).by(1)
  end

  it "attaches selected document templates scoped to the workspace" do
    sign_in(u)
    mine = create(:document_template, workspace: ws)
    other = create(:document_template, workspace: create(:workspace))
    post settings_email_templates_path, params: {
      email_template: { name: "X", body_html: "<p>x</p>", document_template_ids: [ mine.id, other.id ] }
    }
    template = EmailTemplate.last
    expect(template.document_template_ids).to eq([ mine.id ])
  end

  it "renders errors for an invalid template" do
    sign_in(u)
    post settings_email_templates_path, params: { email_template: { name: "" } }
    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "destroys a template" do
    sign_in(u)
    t = create(:email_template, workspace: ws)
    expect { delete settings_email_template_path(t) }.to change(EmailTemplate, :count).by(-1)
  end

  it "enqueues regeneration" do
    sign_in(u)
    t = create(:email_template, workspace: ws)
    expect { post regenerate_settings_email_template_path(t) }.to have_enqueued_job(EmailTemplateGenerationJob)
  end
end
