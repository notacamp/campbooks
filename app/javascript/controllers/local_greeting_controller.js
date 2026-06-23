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
  static values = { greetings: Object }

  connect() {
    const bucket = this.bucketFor(new Date().getHours())

    const text = this.greetingsValue[bucket]
    if (text && this.hasTextTarget) this.textTarget.textContent = text

    this.iconTargets.forEach((el) => {
      el.classList.toggle("hidden", el.dataset.bucket !== bucket)
    })
  }

  bucketFor(hour) {
    if (hour >= 5 && hour < 12) return "morning"
    if (hour >= 12 && hour < 17) return "afternoon"
    if (hour >= 17 && hour < 22) return "evening"
    return "night"
  }
}
