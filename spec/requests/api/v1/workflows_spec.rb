require "rails_helper"

RSpec.describe "API v1 workflows", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def read_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "workflows:read")
  end

  def trigger_headers
    api_auth_headers(workspace: workspace, user: user, scopes: "workflows:trigger")
  end

  describe "GET /api/v1/workflows" do
    it "lists only the workspace's workflows" do
      mine = create(:workflow, workspace: workspace, name: "My flow")
      create(:workflow, workspace: create(:workspace)) # another workspace

      get api_v1_workflows_path, headers: read_headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"].map { |w| w["id"] }).to eq([ mine.id ])
      expect(body["data"].first).to include("name" => "My flow", "trigger_type" => "email_received")
      expect(body["meta"]).to include("total" => 1)
    end

    it "401s without a token" do
      get api_v1_workflows_path
      expect(response).to have_http_status(:unauthorized)
    end

    it "403s without the workflows:read scope" do
      get api_v1_workflows_path,
          headers: api_auth_headers(workspace: workspace, user: user, scopes: "emails:read")
      expect(response).to have_http_status(:forbidden)
    end
  end

  describe "POST /api/v1/workflows/:id/trigger" do
    it "enqueues the webhook job with the payload and returns 202" do
      wf = create(:workflow, :webhook, workspace: workspace)
      allow(WorkflowWebhookJob).to receive(:perform_later)

      post trigger_api_v1_workflow_path(wf),
           params: { payload: { status: "paid" } }, headers: trigger_headers

      expect(response).to have_http_status(:accepted)
      expect(response.parsed_body.dig("data", "ok")).to be(true)
      expect(WorkflowWebhookJob).to have_received(:perform_later)
        .with(wf.id, hash_including(payload: hash_including("status" => "paid")))
    end

    it "422s for a non-webhook workflow" do
      wf = create(:workflow, workspace: workspace) # email_received
      allow(WorkflowWebhookJob).to receive(:perform_later)

      post trigger_api_v1_workflow_path(wf), headers: trigger_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("not_triggerable")
      expect(WorkflowWebhookJob).not_to have_received(:perform_later)
    end

    it "422s for a disabled webhook workflow" do
      wf = create(:workflow, :webhook, workspace: workspace, enabled: false)
      post trigger_api_v1_workflow_path(wf), headers: trigger_headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("workflow_disabled")
    end

    it "403s with only the read scope" do
      wf = create(:workflow, :webhook, workspace: workspace)
      post trigger_api_v1_workflow_path(wf), headers: read_headers
      expect(response).to have_http_status(:forbidden)
    end

    it "404s for a workflow in another workspace" do
      wf = create(:workflow, :webhook, workspace: create(:workspace))
      post trigger_api_v1_workflow_path(wf), headers: trigger_headers
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "GET /api/v1/workflows/:workflow_id/executions" do
    it "lists the run history newest-first" do
      wf = create(:workflow, :webhook, workspace: workspace)
      older = wf.executions.create!(workspace: workspace, status: :completed, created_at: 2.hours.ago)
      newer = wf.executions.create!(workspace: workspace, status: :failed,
                                    error_message: "boom", created_at: 1.hour.ago)

      get api_v1_workflow_executions_path(wf), headers: read_headers

      expect(response).to have_http_status(:ok)
      data = response.parsed_body["data"]
      expect(data.map { |e| e["id"] }).to eq([ newer.id, older.id ])
      expect(data.first).to include("status" => "failed", "error_message" => "boom")
    end

    it "404s for a workflow in another workspace" do
      wf = create(:workflow, :webhook, workspace: create(:workspace))
      get api_v1_workflow_executions_path(wf), headers: read_headers
      expect(response).to have_http_status(:not_found)
    end
  end
end
