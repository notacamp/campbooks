require "rails_helper"

RSpec.describe "Webhooks", type: :request do
  let(:workflow) { create(:workflow, :webhook) }

  before { allow(WorkflowWebhookJob).to receive(:perform_later) }

  it "accepts a JSON POST and enqueues the workflow with the parsed payload" do
    post webhook_path(workflow.webhook_token),
         params: { event: "invoice.paid", amount: 42 }.to_json,
         headers: { "CONTENT_TYPE" => "application/json" }

    expect(response).to have_http_status(:accepted)
    expect(JSON.parse(response.body)).to include("ok" => true)
    expect(WorkflowWebhookJob).to have_received(:perform_later)
      .with(workflow.id, hash_including(payload: { "event" => "invoice.paid", "amount" => 42 }))
  end

  it "captures query parameters and source IP" do
    post webhook_path(workflow.webhook_token, source: "stripe"),
         params: {}.to_json,
         headers: { "CONTENT_TYPE" => "application/json" }

    expect(WorkflowWebhookJob).to have_received(:perform_later)
      .with(workflow.id, hash_including(query: { "source" => "stripe" }))
  end

  it "does not persist sensitive headers" do
    post webhook_path(workflow.webhook_token),
         params: {}.to_json,
         headers: { "CONTENT_TYPE" => "application/json", "Authorization" => "Bearer secret" }

    expect(WorkflowWebhookJob).to have_received(:perform_later) do |_id, kwargs|
      expect(kwargs[:headers]).not_to have_key("Authorization")
    end
  end

  it "returns 404 for an unknown token" do
    post webhook_path("does-not-exist")
    expect(response).to have_http_status(:not_found)
    expect(WorkflowWebhookJob).not_to have_received(:perform_later)
  end

  it "returns 404 for a disabled workflow" do
    workflow.update!(enabled: false)
    post webhook_path(workflow.webhook_token)
    expect(response).to have_http_status(:not_found)
  end

  it "is reachable without authentication" do
    post webhook_path(workflow.webhook_token), params: {}.to_json, headers: { "CONTENT_TYPE" => "application/json" }
    expect(response).not_to redirect_to("/session/new")
    expect(response).to have_http_status(:accepted)
  end
end
