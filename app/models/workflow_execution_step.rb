class WorkflowExecutionStep < ApplicationRecord
  belongs_to :workflow_execution
  belongs_to :workflow_step

  enum :status, { pending: 0, running: 1, completed: 2, failed: 3, skipped: 4 }
end
