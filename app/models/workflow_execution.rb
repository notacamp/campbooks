class WorkflowExecution < ApplicationRecord
  belongs_to :workflow
  belongs_to :workspace

  has_many :execution_steps, class_name: "WorkflowExecutionStep", dependent: :destroy

  enum :status, { running: 0, completed: 1, failed: 2 }
end
