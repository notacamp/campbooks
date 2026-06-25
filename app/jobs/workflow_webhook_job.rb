# Runs a webhook-triggered workflow off the request thread. The inbound request
# is decomposed into plain hashes by WebhooksController so the job stays
# serializable and the workflow can read `payload` / `headers` / `query`.
class WorkflowWebhookJob < ApplicationJob
  queue_as :default

  def perform(workflow_id, payload: {}, headers: {}, query: {}, source_ip: nil)
    return unless Features.workflows?

    workflow = Workflow.enabled.find_by(id: workflow_id)
    return unless workflow&.webhook?

    context = Workflows::WebhookContext.new(
      payload: payload,
      headers: headers,
      query: query,
      source_ip: source_ip
    )

    Workflows::Executor.call(workflow, context)
  end
end
