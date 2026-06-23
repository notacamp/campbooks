module Calendars
  # Async wrapper around EventWriter — keeps the controller response fast and lets
  # Solid Queue retry transient provider failures with backoff. The local event is
  # already saved with outbound_pending: true, so on a provider outage state stays
  # consistent and the write simply retries.
  class EventWriteJob < ApplicationJob
    queue_as :default
    retry_on StandardError, wait: :polynomially_longer, attempts: 3
    discard_on ActiveJob::DeserializationError # event deleted before the job ran
    discard_on AuthenticationError # dead grant — token refresh will deactivate it

    def perform(event_id, operation, scope = "this")
      event = CalendarEvent.find_by(id: event_id)
      return unless event

      Calendars::EventWriter.new(event).call(operation, scope: scope.to_sym)
    rescue Calendars::ConflictError => e
      # The writer already tried to resolve; if it still conflicts, stop spinning —
      # clear the flag and let the next inbound sync reconcile the row.
      CalendarEvent.where(id: event_id).update_all(outbound_pending: false)
      Rails.logger.warn("[Calendars::EventWriteJob] unresolved conflict for event #{event_id}: #{e.message}")
    end
  end
end
