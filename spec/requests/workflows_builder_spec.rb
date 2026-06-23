require "rails_helper"

# Renders the real builder view (Campbooks::StepPicker + Campbooks::WorkflowStepForm)
# to prove the registry-derived catalog and action <select> options render after
# the Layer 0 refactor — without depending on the browser login.
RSpec.describe "Workflows builder rendering", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:workflow) { create(:workflow, workspace: workspace) }

  before { sign_in(user) }

  it "renders the edit page with every registry action label and the condition card" do
    workflow.steps.create!(position: 0, step_type: "action", action_type: "http_request", config: {})

    get edit_workflow_path(workflow)

    expect(response).to have_http_status(:ok)
    # The step picker lists all four action cards regardless of existing steps;
    # each label is sourced from Workflows::ActionRegistry.
    Workflows::ActionRegistry.labels.each_value do |label|
      expect(response.body).to include(label)
    end
    expect(response.body).to include("Condition")
  end
end
