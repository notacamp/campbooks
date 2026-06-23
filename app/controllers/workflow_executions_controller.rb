class WorkflowExecutionsController < ApplicationController
  before_action :require_authentication
  before_action :set_workflow

  def index
    @executions = @workflow.executions
                           .includes(execution_steps: :workflow_step)
                           .limit(100)
  end

  def show
    @execution = @workflow.executions.find(params[:id])
    @steps = @execution.execution_steps.includes(:workflow_step).order(:created_at)
  end

  private

  def set_workflow
    @workflow = Current.workspace.workflows.find(params[:workflow_id])
  end
end
