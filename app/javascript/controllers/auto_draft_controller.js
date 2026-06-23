import { Controller } from "@hotwired/stimulus"

// Auto-submits the form it's attached to when the page was opened with an intent
// flag in the URL (?compose=follow_up), then strips the flag so a Turbo back/forward
// cache restore can't re-fire it. This turns the follow-up card's "Draft follow-up"
// into one tap: the thread opens and the AI draft kicks straight off into the
// composer, instead of the user having to tap the chip again. With no flag present
// the chip stays a normal manual button.
export default class extends Controller {
  static values = {
    param: { type: String, default: "compose" },
    match: { type: String, default: "follow_up" }
  }

  connect() {
    const url = new URL(window.location.href)
    if (url.searchParams.get(this.paramValue) !== this.matchValue) return

    // Strip the flag first so reconnecting (cache restore) is a no-op.
    url.searchParams.delete(this.paramValue)
    window.history.replaceState({}, "", url.pathname + url.search + url.hash)

    // Defer one frame so Turbo has fully wired the form before we submit it.
    requestAnimationFrame(() => {
      if (typeof this.element.requestSubmit === "function") this.element.requestSubmit()
      else this.element.submit()
    })
  }
}
