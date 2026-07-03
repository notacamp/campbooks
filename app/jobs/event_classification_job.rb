class EventClassificationJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  # Auto-classifies a freshly-created event into an EventType (whose icon labels
  # it on the grids — purely local, nothing to push to the provider).
  # Idempotent: only runs while the event is still `pending` (the user hasn't
  # manually chosen a type/none), so a manual override is never overwritten.
  def perform(event_id)
    event = CalendarEvent.find(event_id)
    return unless event.type_status_pending?

    workspace = event.workspace
    Current.workspace = workspace

    # No text provider → leave the event untyped (it just shows no icon).
    return unless Ai::ProviderSetup.configured?(workspace, :text)

    type = Ai::EventClassifier.new(event).call
    # `:auto` even on a no-match so the job never re-runs for this event.
    event.update!(event_type: type, type_status: :auto)
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[EventClassificationJob] CalendarEvent #{event_id} not found, skipping")
  rescue => e
    Rails.logger.error("[EventClassificationJob] Error classifying event #{event_id}: #{e.message}")
    raise
  ensure
    Current.workspace = nil
  end
end
