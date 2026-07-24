import { Controller } from "@hotwired/stimulus"

// Manages the chip date/time row on the calendar event form:
//   - Tracks the event duration so it is preserved when the start is moved.
//   - Shifts the end when the start changes (keeping duration).
//   - Clamps the end to start+30min when the user moves it before the start.
//   - Hides the time chip spans (start time / dash / end time) for all-day events.

export default class extends Controller {
  static targets = [
    "startDate", "startTime",
    "endDate",   "endTime",
    "timeFields",
    "alldayToggle"
  ]

  connect() {
    this._durationMs = this.durationMs()
    // Apply all-day visibility immediately so pre-checked toggles hide time chips.
    this.alldayChanged()
  }

  // ── All-day ───────────────────────────────────────────────────────────────

  alldayChanged() {
    const allDay = this.alldayToggleTarget.checked
    this.timeFieldsTargets.forEach(el => el.classList.toggle("hidden", allDay))
  }

  // ── Start changed ─────────────────────────────────────────────────────────

  startChanged() {
    const startDate = this.startDateTarget.value
    const startTime = this.startTimeTarget.value || "00:00"
    if (!startDate) return

    const start = new Date(`${startDate}T${startTime}`)
    if (isNaN(start.getTime())) return

    const newEnd = new Date(start.getTime() + this._durationMs)
    this.endDateTarget.value = this.formatDate(newEnd)
    this.endTimeTarget.value = this.formatTime(newEnd)

    this._durationMs = this.durationMs()
  }

  // ── End changed ───────────────────────────────────────────────────────────

  endChanged() {
    const startDate = this.startDateTarget.value
    const startTime = this.startTimeTarget.value || "00:00"
    const endDate   = this.endDateTarget.value
    const endTime   = this.endTimeTarget.value   || "00:00"
    if (!startDate || !endDate) return

    const start = new Date(`${startDate}T${startTime}`)
    const end   = new Date(`${endDate}T${endTime}`)
    if (isNaN(start.getTime()) || isNaN(end.getTime())) return

    if (end <= start) {
      const clamped = new Date(start.getTime() + 30 * 60 * 1000)
      this.endDateTarget.value = this.formatDate(clamped)
      this.endTimeTarget.value = this.formatTime(clamped)
    }

    this._durationMs = this.durationMs()
  }

  // ── Duration ─────────────────────────────────────────────────────────────

  durationMs() {
    const startDate = this.startDateTarget.value
    const startTime = this.startTimeTarget.value || "00:00"
    const endDate   = this.endDateTarget.value
    const endTime   = this.endTimeTarget.value   || "00:00"

    if (!startDate || !endDate) return 3_600_000

    const start = new Date(`${startDate}T${startTime}`)
    const end   = new Date(`${endDate}T${endTime}`)

    if (isNaN(start.getTime()) || isNaN(end.getTime())) return 3_600_000

    const ms = end - start
    return ms > 0 ? ms : 3_600_000
  }

  // ── Formatters ───────────────────────────────────────────────────────────

  formatDate(date) {
    const y = date.getFullYear()
    const m = String(date.getMonth() + 1).padStart(2, "0")
    const d = String(date.getDate()).padStart(2, "0")
    return `${y}-${m}-${d}`
  }

  formatTime(date) {
    const h = String(date.getHours()).padStart(2, "0")
    const m = String(date.getMinutes()).padStart(2, "0")
    return `${h}:${m}`
  }
}
