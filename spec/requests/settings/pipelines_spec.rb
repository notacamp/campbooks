require "rails_helper"

RSpec.describe "Settings::Pipelines", type: :request do
  let(:user) { create(:user) }
  let(:workspace) { user.workspace }

  before { sign_in(user) }

  def valid_params(name: "Invoices")
    {
      pipeline: {
        name: name, applies_to: "documents",
        stages_attributes: { "0" => { name: "New", color: "#6366f1", position: "1" } }
      }
    }
  end

  describe "GET /settings/pipelines" do
    it "renders the list" do
      create(:pipeline, :with_stages, workspace: workspace, name: "Onboarding")
      get settings_pipelines_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Onboarding")
    end
  end

  describe "POST /settings/pipelines" do
    it "creates a pipeline with stages on the Free plan" do
      expect { post settings_pipelines_path, params: valid_params }
        .to change(workspace.pipelines, :count).by(1)
      expect(workspace.pipelines.last.stages.count).to eq(1)
      expect(response).to redirect_to(settings_pipelines_path)
    end
  end

  describe "Free plan at the 1-pipeline cap" do
    before { create(:pipeline, :with_stages, workspace: workspace, name: "Existing") }

    it "blocks creating a second pipeline" do
      expect { post settings_pipelines_path, params: valid_params(name: "Second") }
        .not_to change(Pipeline, :count)
    end

    it "still lets the user view, edit and delete the one they have" do
      existing = workspace.pipelines.first

      get settings_pipelines_path
      expect(response).to have_http_status(:ok)

      get edit_settings_pipeline_path(existing)
      expect(response).to have_http_status(:ok)

      expect { delete settings_pipeline_path(existing) }.to change(Pipeline, :count).by(-1)
    end
  end

  describe "when the plan does not include pipelines" do
    before { workspace.update!(entitlement_overrides: { "pipelines" => { "allowed" => false } }) }

    it "blocks the new form but still renders the index (with an upgrade prompt)" do
      get new_settings_pipeline_path
      expect(response).to have_http_status(:redirect)

      get settings_pipelines_path
      expect(response).to have_http_status(:ok)
    end
  end

  describe "PATCH /settings/pipelines/:id" do
    it "updates the pipeline" do
      pipeline = create(:pipeline, :with_stages, workspace: workspace, name: "Old")
      patch settings_pipeline_path(pipeline), params: { pipeline: { name: "New name" } }
      expect(pipeline.reload.name).to eq("New name")
    end
  end

  it "scopes to the current workspace (404 for another workspace's pipeline)" do
    other = create(:pipeline, workspace: create(:workspace))
    get edit_settings_pipeline_path(other)
    expect(response).to have_http_status(:not_found)
  end
end
