import { Controller } from "@hotwired/stimulus"

// Computes the next matching local datetime for the chosen frequency/weekday/time
// and writes it (ISO-8601 with UTC offset) into the hidden digest[first_run_at]
// field so the server receives a full, unambiguous datetime.
//
// Data attributes on the wrapping element (set by Campbooks::Digests::Form):
//   data-digest-schedule-freq          initial frequency e.g. "FREQ=WEEKLY"
//   data-digest-schedule-wday          initial weekday (0-6)
//   data-digest-schedule-hour          initial hour (two-digit string)
//   data-digest-schedule-min           initial minute (two-digit string)
//   data-digest-schedule-existing-iso  ISO-8601 of the existing next_run_at (edit)
export default class extends Controller {
  static targets = [
    "frequency",
    "weekdayRow",
    "weekday",
    "hour",
    "minute",
    "firstRunAt",
    "preview",
    "submit",
  ]

  connect() {
    const existing = this.element.dataset.digestScheduleExistingIso
    if (existing) {
      const d = new Date(existing)
      if (!isNaN(d.getTime())) {
        if (this.hasHourTarget)    this.hourTarget.value    = String(d.getHours()).padStart(2, "0")
        if (this.hasMinuteTarget)  this.minuteTarget.value  = String(d.getMinutes()).padStart(2, "0")
        if (this.hasWeekdayTarget) this.weekdayTarget.value = String(d.getDay())
      }
    } else {
      const { digestScheduleWday: wday, digestScheduleHour: hour, digestScheduleMin: min } =
        this.element.dataset
      if (wday !== undefined && this.hasWeekdayTarget) this.weekdayTarget.value = wday
      if (hour !== undefined && this.hasHourTarget)    this.hourTarget.value    = hour
      if (min  !== undefined && this.hasMinuteTarget)  this.minuteTarget.value  = min
    }

    this.update()
  }

  update() {
    this.toggleWeekdayVisibility()
    const next = this.computeNext()
    if (!next) return

    if (this.hasFirstRunAtTarget) this.firstRunAtTarget.value = next.toISOString()
    if (this.hasPreviewTarget)    this.previewTarget.textContent = this.formatPreview(next)
  }

  toggleWeekdayVisibility() {
    if (!this.hasWeekdayRowTarget) return
    const freq = this.hasFrequencyTarget ? this.frequencyTarget.value : "FREQ=WEEKLY"
    this.weekdayRowTarget.hidden = !freq.includes("WEEKLY")
  }

  computeNext() {
    const freq = this.hasFrequencyTarget ? this.frequencyTarget.value : "FREQ=WEEKLY"
    const hour = parseInt(this.hasHourTarget   ? this.hourTarget.value   : "8",  10) || 0
    const min  = parseInt(this.hasMinuteTarget ? this.minuteTarget.value : "0",  10) || 0
    const wday = parseInt(this.hasWeekdayTarget ? this.weekdayTarget.value : "1", 10)

    const now = new Date()
    const candidate = new Date(now.getFullYear(), now.getMonth(), now.getDate(), hour, min, 0, 0)

    if (freq.includes("WEEKLY")) {
      let daysAhead = ((wday - candidate.getDay()) + 7) % 7
      if (daysAhead === 0 && candidate <= now) daysAhead = 7
      candidate.setDate(candidate.getDate() + daysAhead)
    } else if (freq.includes("MONTHLY")) {
      if (candidate <= now) candidate.setMonth(candidate.getMonth() + 1)
    } else {
      // Daily
      if (candidate <= now) candidate.setDate(candidate.getDate() + 1)
    }

    return candidate
  }

  formatPreview(date) {
    try {
      return date.toLocaleString(undefined, {
        weekday: "long",
        year:    "numeric",
        month:   "long",
        day:     "numeric",
        hour:    "2-digit",
        minute:  "2-digit",
      })
    } catch (_e) {
      return date.toString()
    }
  }
}
