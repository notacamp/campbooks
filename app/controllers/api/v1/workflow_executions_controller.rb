# frozen_string_literal: true

module Api
  module V1
    # Read-only run history for a workflow. Nested under /workflows/:workflow_id.
    # Workspace-scoped via the parent workflow lookup (404 if it isn't visible).
    class WorkflowExecutionsController < BaseController
      before_action -> { doorkeeper_authorize! :"workflows:read" }, only: :index
      before_action :set_workflow

      def index
        # The has_many already orders created_at: :desc (newest first).
        @pagy, executions = pagy(@workflow.executions, limit: per_page)
        render_page(executions.map { |execution| WorkflowExecutionSerializer.new(execution).as_json }, @pagy)
      end

      private

      def set_workflow
        @workflow = Current.workspace.workflows.find(params[:workflow_id])
      end
    end
  end
end
