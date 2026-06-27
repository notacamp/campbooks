require "rails_helper"
RSpec.describe "Settings::DocumentTemplates", type: :request do
  let(:ws) { create(:workspace) }
  let(:u) { create(:user, workspace: ws) }
  before { allow(Features).to receive(:document_templates?).and_return(true) }
  it "requires auth" do
    get settings_document_templates_path
    expect(response).to redirect_to(new_session_path)
  end
  it "returns ok" do
    sign_in(u); get settings_document_templates_path
    expect(response).to have_http_status(:ok)
  end
  it "404 when disabled" do
    allow(Features).to receive(:document_templates?).and_return(false)
    sign_in(u); get settings_document_templates_path
    expect(response).to have_http_status(:not_found)
  end
  it "creates template" do
    sign_in(u)
    expect{post settings_document_templates_path, params:{document_template:{name:"X",description:"d"}}}.to change(DocumentTemplate,:count).by(1)
  end
  it "renders errors" do
    sign_in(u); post settings_document_templates_path, params:{document_template:{name:""}}
    expect(response).to have_http_status(:unprocessable_entity)
  end
  it "destroys" do
    sign_in(u); t=create(:document_template, workspace:ws)
    expect{delete settings_document_template_path(t)}.to change(DocumentTemplate,:count).by(-1)
  end
  it "enqueues regenerate" do
    sign_in(u); t=create(:document_template, workspace:ws)
    expect{post regenerate_settings_document_template_path(t)}.to have_enqueued_job(DocumentTemplateGenerationJob)
  end
end
