import { Controller } from "@hotwired/stimulus"

// One-shot navigator: visits urlValue the moment it connects. Rendered by a Turbo
// Stream after a successful calendar-event save to break out of the modal's frame
// and land on the calendar at the event's date, so the new/updated event is
// visible (mirrors calendar-dnd's post-reschedule Turbo.visit refresh).
export default class extends Controller {
  static values = { url: String, action: { type: String, default: "advance" } }

  connect() {
    if (this.urlValue) Turbo.visit(this.urlValue, { action: this.actionValue })
  }
}
