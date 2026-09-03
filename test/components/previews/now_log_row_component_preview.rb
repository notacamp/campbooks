# frozen_string_literal: true

# One row of Scout's log: a mono timestamp, the event's sentence, and the row's
# actions — Undo where a reverse exists (archive/tag), Open where the subject has
# a page, and the muted post-undo state.
class NowLogRowComponentPreview < ViewComponent::Preview
  # Reversible: an archived email → Undo + Open.
  def reversible
    render(Campbooks::Now::LogRow.new(event: build_event(
      name: "email.archived", subject_type: "EmailMessage", subject_id: 42,
      payload: { "subject" => "Q3 kickoff deck: feedback by Friday?" })))
  end

  # Non-reversible: a processed document → Open only.
  def open_only
    render(Campbooks::Now::LogRow.new(event: build_event(
      name: "document.processed", subject_type: "Document", subject_id: 7,
      payload: { "filename" => "cloudhost-invoice.pdf" })))
  end

  # The state the undo turbo-stream swaps in.
  def undone
    render(Campbooks::Now::LogRow.new(event: build_event(
      name: "email.archived", subject_type: "EmailMessage", subject_id: 42), undone: true))
  end

  private

  # An unsaved Event with a stable id — enough for the row's dom_id, label and
  # links without touching the database.
  def build_event(**attrs)
    Event.new({ id: "preview-event", occurred_at: 2.hours.ago, payload: {} }.merge(attrs))
  end
end
