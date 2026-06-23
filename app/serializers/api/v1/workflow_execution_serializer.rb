# frozen_string_literal: true

module Api
  module V1
    # Serializes a WorkflowExecution (one run) for the public API. trigger_data is
    # the payload/context the run started from — useful for debugging — and is the
    # client's own data.
    class WorkflowExecutionSerializer
      def initialize(execution)
        @execution = execution
      end

      def as_json
        {
          id: @execution.id,
          workflow_id: @execution.workflow_id,
          status: @execution.status,
          started_at: @execution.started_at&.iso8601,
          completed_at: @execution.completed_at&.iso8601,
          error_message: @execution.error_message,
          trigger_data: @execution.trigger_data,
          created_at: @execution.created_at.iso8601
        }
      end
    end
  end
end
