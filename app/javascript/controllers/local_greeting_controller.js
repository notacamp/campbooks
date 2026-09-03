import { Controller } from "@hotwired/stimulus"

// Corrects the home greeting to the visitor's DEVICE-LOCAL time of day.
//
// The server renders a best-effort greeting from its own clock (often UTC in
// production), which is wrong for anyone in another timezone. On connect we read
// the real local hour from `new Date()` and swap in the matching headline + icon
// — no IP lookup, no timezone cookie, no permission prompt.
//
// The thresholds in bucketFor() MUST match
// Campbooks::TimeOfDayGreeting#default_bucket.
export default class extends Controller {
  static targets = ["text", "icon"]
  // saveZoneUrl is set only when the user has no time_zone yet (TimeOfDayGreeting):
  // its presence is the signal to capture the device zone once.
  static values = { greetings: Object, saveZoneUrl: String }

  connect() {
    const bucket = this.bucketFor(new Date().getHours())

    const text = this.greetingsValue[bucket]
    if (text && this.hasTextTarget) this.textTarget.textContent = text

    this.iconTargets.forEach((el) => {
      el.classList.toggle("hidden", el.dataset.bucket !== bucket)
    })

    this.captureZone()
  }

  // Fire-and-forget: report the device's IANA zone so the Time surface can bucket
  // days and find focus slots in the user's own zone. Only runs when the server
  // asked (saveZoneUrl present ⇒ the stored zone is still blank); the endpoint
  // ignores a repeat or an unresolvable zone. Never blocks the greeting.
  captureZone() {
    if (!this.hasSaveZoneUrlValue || !this.saveZoneUrlValue) return

    let zone
    try {
      zone = Intl.DateTimeFormat().resolvedOptions().timeZone
    } catch (_) {
      return
    }
    if (!zone) return

    const token = document.querySelector('meta[name="csrf-token"]')?.content
    fetch(this.saveZoneUrlValue, {
      method: "PATCH",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": token, Accept: "application/json" },
      body: JSON.stringify({ time_zone: zone })
    }).catch(() => {})
  }

  bucketFor(hour) {
    if (hour >= 5 && hour < 12) return "morning"
    if (hour >= 12 && hour < 17) return "afternoon"
    if (hour >= 17 && hour < 22) return "evening"
    return "night"
  }
}
