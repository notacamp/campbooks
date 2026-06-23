# frozen_string_literal: true

# Previews for the workspace activity-feed row. In-memory Event records (with
# built actors/payloads) stand in for real rows, covering the system, user,
# bulk, and custom-event variants plus the unregistered-name fallback.
class ActivityEventRowPreview < ViewComponent::Preview
  # A system-originated ingest event (no actor → "System").
  def system_event
    render Campbooks::Activity::EventRow.new(event: event(
      name: "email.received", actor: nil,
      payload: { "subject" => "Invoice #2025-114 for July booking", "from" => "billing@maplelodge.com" }
    ))
  end

  # A user-actioned event with a linked subject (Document) and a named actor.
  def user_event
    render Campbooks::Activity::EventRow.new(event: event(
      name: "document.approved", actor: User.new(name: "Ada Lovelace"),
      subject_type: "Document", subject_id: 1,
      payload: { "filename" => "maple-lodge-invoice.pdf", "document_type" => "Invoice" }
    ))
  end

  def contact_starred
    render Campbooks::Activity::EventRow.new(event: event(
      name: "contact.starred", actor: User.new(name: "Ada Lovelace"),
      payload: { "name" => "Maple Lodge", "email" => "billing@maplelodge.com" }
    ))
  end

  # A summarizing bulk event (uses the pluralized count fallback).
  def bulk_event
    render Campbooks::Activity::EventRow.new(event: event(
      name: "email.bulk_archived", actor: User.new(name: "Ada Lovelace"),
      payload: { "count" => 12, "ids" => [] }
    ))
  end

  # An unregistered custom event (e.g. from a workflow emit_event) — falls back
  # to a humanized label and the default icon.
  def custom_event
    render Campbooks::Activity::EventRow.new(event: event(
      name: "invoice.flagged", actor: nil,
      payload: { "title" => "Possible duplicate charge" }
    ))
  end

  private

  def event(name:, payload:, actor: nil, subject_type: nil, subject_id: nil)
    Event.new(
      name: name, payload: payload, actor: actor,
      subject_type: subject_type, subject_id: subject_id,
      occurred_at: 2.hours.ago
    )
  end
end
