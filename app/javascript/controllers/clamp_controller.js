import { Controller } from "@hotwired/stimulus"

// Read-more / show-less for clamped text on the home-feed cards (Scout's read and
// the email snippet). Progressive enhancement: the server renders the text
// clamped to N lines with the toggle hidden; on connect we measure whether the
// text actually overflows and only then reveal the button — so short reads never
// show a "Read more" that does nothing, and with no JS the text simply stays
// clamped (still readable).
//
// The clamp lives in the element's inline style (display:-webkit-box +
// -webkit-line-clamp), so expanding is just lifting those properties here — no
// Tailwind arbitrary `line-clamp-[N]` class to keep generated and in sync.
export default class extends Controller {
  static targets = ["content", "button", "more", "less"]
  static values = { lines: { type: Number, default: 10 } }

  connect() {
    this.expanded = false
    this.measure()
    // The clamp height tracks the card width, so re-check on resize (rotate, pane
    // drag) while collapsed — wrapping may cross the line threshold either way.
    this.ro = new ResizeObserver(() => { if (!this.expanded) this.measure() })
    this.ro.observe(this.contentTarget)
  }

  disconnect() {
    this.ro?.disconnect()
  }

  // Show the toggle only when the clamped text is actually cut off.
  measure() {
    const el = this.contentTarget
    const overflowing = el.scrollHeight > el.clientHeight + 1
    this.buttonTarget.classList.toggle("hidden", !overflowing)
  }

  toggle() {
    this.expanded = !this.expanded
    const el = this.contentTarget
    if (this.expanded) {
      el.style.webkitLineClamp = "unset"
      el.style.display = "block"
      el.style.overflow = "visible"
    } else {
      el.style.webkitLineClamp = String(this.linesValue)
      el.style.display = "-webkit-box"
      el.style.overflow = "hidden"
      this.measure()
    }
    this.buttonTarget.setAttribute("aria-expanded", String(this.expanded))
    this.moreTarget.classList.toggle("hidden", this.expanded)
    this.lessTarget.classList.toggle("hidden", !this.expanded)
  }
}
