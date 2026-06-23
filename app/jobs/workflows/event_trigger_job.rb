module Workflows
  # Fans a published Event out to every enabled workflow listening for it via the
  # generic `event` trigger. Mirrors WorkflowTriggerJob (the email path).
  #
  # A workflow's trigger_config["event_name"] selects which events it wants:
  # an exact name ("document.approved") or a prefix wildcard ("document.*").
  #
  # Loop guard: events emitted by a workflow's emit_event action carry an
  # incremented depth; once a causation chain reaches Event::MAX_CHAIN_DEPTH we
  # stop fanning it out, so an emit_event that re-fires its own trigger can't
  # loop forever.
  class EventTriggerJob < ApplicationJob
    queue_as :default

    def perform(event_id)
      event = Event.find_by(id: event_id)
      return unless event
      return if event.depth >= Event::MAX_CHAIN_DEPTH

      workflows = event.workspace.workflows.enabled.where(trigger_type: "event")

      workflows.each do |workflow|
        next unless event_matches?(workflow, event)

        Workflows::Executor.call(workflow, Workflows::EventContext.new(event))
      end
    end

    private

    def event_matches?(workflow, event)
      pattern = workflow.trigger_config.with_indifferent_access[:event_name].to_s
      return false if pattern.blank?
      return true if pattern == event.name

      # Prefix wildcard: "document.*" matches "document.approved".
      if pattern.end_with?(".*")
        prefix = pattern.delete_suffix("*")
        return event.name.start_with?(prefix)
      end

      false
    end
  end
end
