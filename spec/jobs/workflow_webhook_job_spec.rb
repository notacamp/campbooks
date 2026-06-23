require "rails_helper"

RSpec.describe WorkflowWebhookJob, type: :job do
  let(:workflow) { create(:workflow, :webhook) }

  it "runs the executor with a webhook context built from the request data" do
    expect(Workflows::Executor).to receive(:call) do |wf, context|
      expect(wf).to eq(workflow)
      expect(context).to be_a(Workflows::WebhookContext)
      expect(context.payload).to eq("event" => "ping")
      expect(context.source_ip).to eq("1.2.3.4")
    end

    described_class.perform_now(workflow.id, payload: { "event" => "ping" }, source_ip: "1.2.3.4")
  end

  it "does nothing for a disabled workflow" do
    workflow.update!(enabled: false)
    expect(Workflows::Executor).not_to receive(:call)
    described_class.perform_now(workflow.id, payload: {})
  end

  it "does nothing for a non-webhook workflow" do
    email_workflow = create(:workflow, trigger_type: "email_received")
    expect(Workflows::Executor).not_to receive(:call)
    described_class.perform_now(email_workflow.id, payload: {})
  end

  it "does nothing for a missing workflow" do
    expect(Workflows::Executor).not_to receive(:call)
    described_class.perform_now(-1, payload: {})
  end
end
