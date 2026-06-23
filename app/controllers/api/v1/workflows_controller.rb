# frozen_string_literal: true

module Api
  module V1
    # Public API for workflows. Workflows are workspace-scoped (not per-user), so
    # access is gated through Current.workspace. Listing is read-only; triggering
    # is the authenticated equivalent of the public POST /webhooks/:token.
    class WorkflowsController < BaseController
      before_action -> { doorkeeper_authorize! :"workflows:read" },    only: :index
      before_action -> { doorkeeper_authorize! :"workflows:trigger" }, only: :trigger
      before_action :set_workflow, only: :trigger

      def index
        scope = Current.workspace.workflows.order(created_at: :desc)
        @pagy, workflows = pagy(scope, limit: per_page)
        render_page(workflows.map { |workflow| WorkflowSerializer.new(workflow).as_json }, @pagy)
      end

      # Authenticated equivalent of the public webhook endpoint. Only webhook
      # workflows can be fired this way — the runner (WorkflowWebhookJob) no-ops on
      # any other trigger type, so we reject those (and disabled workflows) up front
      # with a clear 422 rather than enqueuing a job that silently does nothing.
      def trigger
        unless @workflow.enabled?
          return render_api_error("workflow_disabled", "This workflow is disabled.",
                                  status: :unprocessable_entity)
        end
        unless @workflow.webhook?
          return render_api_error("not_triggerable",
                                  "Only webhook workflows can be triggered via the API.",
                                  status: :unprocessable_entity)
        end

        WorkflowWebhookJob.perform_later(
          @workflow.id,
          payload: trigger_payload,
          headers: {},
          query: {},
          source_ip: request.remote_ip
        )

        render json: { data: { ok: true, workflow_id: @workflow.id, message: "Workflow triggered." } },
               status: :accepted
      end

      private

      def set_workflow
        @workflow = Current.workspace.workflows.find(params[:id])
      end

      # Optional JSON `payload` object, exposed to the workflow's Liquid templates
      # exactly like the inbound webhook's body. String-keyed to match
      # WebhooksController#extract_payload (JSON.parse). Tolerates absent/non-object.
      def trigger_payload
        raw = params[:payload]
        case raw
        when ActionController::Parameters then raw.to_unsafe_h
        when Hash then raw
        else {}
        end
      end
    end
  end
end
