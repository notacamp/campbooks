class EventClassificationJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  # Auto-classifies a freshly-created event into an EventType (which colors it).
  # Idempotent: only runs while the event is still `pending` (the user hasn't
  # manually chosen a type/none), so a manual override is never overwritten.
  def perform(event_id)
    event = CalendarEvent.find(event_id)
    return unless event.type_status_pending?

    workspace = event.workspace
    Current.workspace = workspace

    # No text provider → leave the event untyped (it keeps the calendar color).
    return unless Ai::ProviderSetup.configured?(workspace, :text)

    type = Ai::EventClassifier.new(event).call
    # `:auto` even on a no-match so the job never re-runs for this event.
    event.update!(event_type: type, type_status: :auto)

    push_color_to_provider(event) if type
  rescue ActiveRecord::RecordNotFound
    Rails.logger.warn("[EventClassificationJob] CalendarEvent #{event_id} not found, skipping")
  rescue => e
    Rails.logger.error("[EventClassificationJob] Error classifying event #{event_id}: #{e.message}")
    raise
  ensure
    Current.workspace = nil
  end

  private

  # Sync the type's color out to the provider — but only when the type color will
  # actually be the rendered color (no per-event override) and the event already
  # exists remotely with a real id and no write in flight. If the create write is
  # still pending (temp "local-" id), the color rides along on it for free, since
  # EventWriter#attrs_for_provider now reads `provider_color`.
  def push_color_to_provider(event)
    return if event.color.present?
    return if event.provider_event_id.to_s.start_with?("local-")
    return if event.outbound_pending?

    event.update!(outbound_pending: true)
    Calendars::EventWriteJob.perform_later(event.id, "update", "this")
  end
end
