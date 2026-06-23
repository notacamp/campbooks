# frozen_string_literal: true

module Api
  module V1
    # Serializes a Workflow for the public API. webhook_token is included (it's the
    # workspace's own shared secret, already shown in the web builder) so a client
    # can construct the public inbound webhook URL when it wants the no-auth path.
    class WorkflowSerializer
      def initialize(workflow)
        @workflow = workflow
      end

      def as_json
        {
          id: @workflow.id,
          name: @workflow.name,
          description: @workflow.description,
          trigger_type: @workflow.trigger_type,
          enabled: @workflow.enabled,
          webhook_token: @workflow.webhook_token,
          created_at: @workflow.created_at.iso8601,
          updated_at: @workflow.updated_at.iso8601
        }
      end
    end
  end
end
