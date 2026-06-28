require "rails_helper"

RSpec.describe "Settings::DocumentTemplates", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { allow(Features).to receive(:document_templates?).and_return(true) }

  it "requires authentication" do
    get settings_document_templates_path
    expect(response).to redirect_to(new_session_path)
  end

  it "404s when the feature flag is off" do
    allow(Features).to receive(:document_templates?).and_return(false)
    sign_in(user)
    get settings_document_templates_path
    expect(response).to have_http_status(:not_found)
  end

  context "with an entitled (paid) plan" do
    before do
      sign_in(user)
      workspace.update!(plan: "pro")
    end

    it "renders the empty index" do
      get settings_document_templates_path
      expect(response).to have_http_status(:ok)
    end

    it "renders the index with a template (status badge + row actions)" do
      create(:document_template, :ai_completed, workspace: workspace)
      get settings_document_templates_path
      expect(response).to have_http_status(:ok)
    end

    it "renders the new form" do
      get new_settings_document_template_path
      expect(response).to have_http_status(:ok)
    end

    it "renders the edit form with the variables table" do
      template = create(:document_template, :ai_completed, workspace: workspace)
      get edit_settings_document_template_path(template)
      expect(response).to have_http_status(:ok)
    end

    it "creates a template" do
      expect { post settings_document_templates_path, params: { document_template: { name: "X", description: "d" } } }
        .to change(DocumentTemplate, :count).by(1)
    end

    it "re-renders new on a validation error" do
      post settings_document_templates_path, params: { document_template: { name: "" } }
      expect(response).to have_http_status(:unprocessable_entity)
    end

    it "destroys a template" do
      template = create(:document_template, workspace: workspace)
      expect { delete settings_document_template_path(template) }.to change(DocumentTemplate, :count).by(-1)
    end

    it "enqueues regeneration and redirects to edit" do
      template = create(:document_template, workspace: workspace)
      expect { post regenerate_settings_document_template_path(template) }
        .to have_enqueued_job(DocumentTemplateGenerationJob)
      expect(response).to redirect_to(edit_settings_document_template_path(template))
    end
  end

  context "on the free plan" do
    before { sign_in(user) }

    it "blocks creating a template (entitlement gate)" do
      expect { post settings_document_templates_path, params: { document_template: { name: "X" } } }
        .not_to change(DocumentTemplate, :count)
      expect(response).to have_http_status(:redirect)
    end
  end
end
