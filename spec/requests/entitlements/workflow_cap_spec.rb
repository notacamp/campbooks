require "rails_helper"

# Workflows are gated by the plan: not included on Free, included with a count cap
# on paid plans. WorkflowsController#create is the safety-net guard behind the
# (also gated) builder UI.
RSpec.describe "Workflow entitlement gate", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  context "on the Free plan" do
    it "blocks creating a workflow and does not persist one" do
      expect { post workflows_path }.not_to change(Workflow, :count)
      expect(response).to have_http_status(:redirect)
    end
  end

  context "on the Pro plan" do
    before { user.workspace.update!(plan: "pro") }

    it "creates a workflow" do
      expect { post workflows_path }.to change(Workflow, :count).by(1)
    end
  end
end
