# frozen_string_literal: true

# Previews for the /digests index row. In-memory ScheduledDigest records (ids set
# so the edit/delete/toggle route helpers resolve).
class DigestRowComponentPreview < ViewComponent::Preview
  # An enabled weekly digest with three sources.
  def enabled
    render Campbooks::Digests::DigestRow.new(digest: digest)
  end

  # A disabled digest — muted badge, toggle off.
  def disabled
    render Campbooks::Digests::DigestRow.new(
      digest: digest(name: "Invoice tracker", enabled: false, sources: [ { "type" => "documents", "document_types" => [ "invoice", "receipt" ] } ])
    )
  end

  # A single-source daily digest.
  def daily_single_source
    render Campbooks::Digests::DigestRow.new(
      digest: digest(name: "Morning inbox brief", rrule: "FREQ=DAILY", sources: [ { "type" => "emails", "query" => "is:unread" } ])
    )
  end

  private

  def digest(name: "Week ahead", rrule: "FREQ=WEEKLY", enabled: true, sources: nil)
    sources ||= [
      { "type" => "calendar", "window_days" => 7 },
      { "type" => "tasks", "window_days" => 7, "include_overdue" => true },
      { "type" => "reminders", "window_days" => 7 }
    ]
    ScheduledDigest.new(
      id: "11111111-1111-4111-8111-111111111111",
      name: name,
      rrule: rrule,
      enabled: enabled,
      next_run_at: 3.days.from_now.change(hour: 18, min: 0),
      config: { "sources" => sources }
    )
  end
end
