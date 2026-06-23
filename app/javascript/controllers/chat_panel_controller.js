import { Controller } from "@hotwired/stimulus"

const KEY = "campbooks:discussion-open"
const MOBILE_BREAKPOINT = 1024

// The desktop Discussion pane. Collapsed by default to a labeled rail (an icon, a
// comment count, and a vertical "Discussion" label) remembered per browser; clicking
// the rail — or the header's collapse button — toggles it. On mobile the pane is
// surfaced as a full-screen overlay by scout-mobile, so there we keep it expanded
// regardless of the desktop preference (the collapse rail is a desktop affordance).
export default class extends Controller {
  static targets = ["body", "header", "rail", "handle"]

  connect() {
    // Arriving with #discussion (e.g. the drawer's "Discussion" button opening
    // the full view) forces the pane open regardless of the saved preference.
    this.open = this.desiredState() || this.requestedByUrl()
    this.apply(false)
    this.onResize = this.onResize.bind(this)
    window.addEventListener("resize", this.onResize)
  }

  requestedByUrl() {
    return window.location.hash === "#discussion"
  }

  disconnect() {
    window.removeEventListener("resize", this.onResize)
  }

  // Mobile always opens (it shows as an overlay); desktop follows the saved preference.
  desiredState() {
    return window.innerWidth < MOBILE_BREAKPOINT ? true : (localStorage.getItem(KEY) === "true")
  }

  // Re-evaluate when the viewport crosses the breakpoint so the overlay never shows
  // the collapsed rail (and the desktop rail returns when widening back).
  onResize() {
    const next = this.desiredState()
    if (next !== this.open) {
      this.open = next
      this.apply(false)
    }
  }

  toggle() {
    this.open = !this.open
    localStorage.setItem(KEY, String(this.open))
    this.apply(true)
  }

  apply(savePrevWidth) {
    if (this.open) {
      this.element.classList.remove("w-12")
      this.element.classList.add("w-96")
      const w = this.element.dataset.openWidth
      if (w) this.element.style.width = `${w}px`
    } else {
      if (savePrevWidth) {
        const cw = this.element.getBoundingClientRect().width
        if (cw > 60) this.element.dataset.openWidth = Math.round(cw)
      }
      this.element.style.width = ""
      this.element.classList.add("w-12")
      this.element.classList.remove("w-96")
    }

    if (this.hasBodyTarget) this.bodyTarget.classList.toggle("hidden", !this.open)
    if (this.hasHeaderTarget) this.headerTarget.classList.toggle("hidden", !this.open)
    if (this.hasRailTarget) this.railTarget.classList.toggle("hidden", this.open)
    // Resizing a 48px rail is meaningless, so the drag handle only exists when open.
    if (this.hasHandleTarget) this.handleTarget.style.display = this.open ? "" : "none"
  }
}
