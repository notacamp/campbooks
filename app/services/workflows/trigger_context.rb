module Workflows
  # A trigger context wraps *whatever* fired a workflow and gives the executor
  # one uniform surface, regardless of whether the source was an inbound email
  # or an external webhook:
  #
  #   liquid_context — Hash exposed to Liquid templates in steps
  #   trigger_data   — Hash persisted on the WorkflowExecution record
  #   step_input     — Hash persisted on each WorkflowExecutionStep
  #   documents      — Documents available to conditions (email only)
  #   email_message  — the originating email, or nil for non-email triggers
  #   subject        — the record the trigger is about (for emit_event to carry
  #                    through), or nil
  #   source_event   — the Event that fired this run (event trigger only), so an
  #                    emit_event action can chain its new event via caused_by
  #
  # Subclasses: EmailContext, WebhookContext, EventContext.
  class TriggerContext
    def liquid_context
      {}
    end

    def trigger_data
      {}
    end

    def step_input
      {}
    end

    def documents
      []
    end

    def email_message
      nil
    end

    def subject
      nil
    end

    def source_event
      nil
    end
  end
end
