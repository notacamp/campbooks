import { Controller } from "@hotwired/stimulus"

// Auto-hides this element when the user scrolls down and reveals it on
// scroll-up. Designed for the mobile bottom nav: the bar slides off-screen to
// reclaim vertical space during downward reading, then glides back as soon as
// the user scrolls up to act or navigate.
//
// Listens on document in capture phase so it catches scroll events from both
// window-level scrollers (application.html.erb pages: home feed, /files, etc.)
// and fixed-height inner-container scrollers (email.html.erb inbox layout).
// A single lastPos tracks the most-recent scrolling element's position; on
// mobile only one pane scrolls at a time, so this is always correct.
//
// A 10 px threshold filters micro-jitter; a 60 px top guard keeps the bar
// visible while the container is near its top.
export default class extends Controller {
  // How many pixels the user must scroll in one direction before we react —
  // filters out micro-jitter and inertia tails.
  static THRESHOLD = 10

  // Scroll position below which the bar is always shown regardless of direction,
  // so it doesn't disappear on the very first tiny scroll.
  static TOP_GUARD = 60

  connect() {
    this.lastPos = window.scrollY
    this.ticking = false
    this.hidden = false

    this.element.classList.add("transition-transform", "duration-300")

    this.onScroll = (e) => {
      // Capture the current position for this specific scroll target right now
      // (before rAF) so we always compare against the element that fired the event.
      const target = e.target
      this._pendingPos = (target === document || target === window)
        ? window.scrollY
        : target.scrollTop

      if (this.ticking) return
      this.ticking = true
      requestAnimationFrame(() => {
        this.ticking = false
        this._update(this._pendingPos)
      })
    }

    // Capture phase: catches scroll on any descendant container as well as window.
    document.addEventListener("scroll", this.onScroll, { passive: true, capture: true })
  }

  disconnect() {
    document.removeEventListener("scroll", this.onScroll, { capture: true })
  }

  _update(pos) {
    const delta = pos - this.lastPos

    if (pos <= this.constructor.TOP_GUARD) {
      // Always show when near the top of the scrolling container.
      this._show()
    } else if (delta > this.constructor.THRESHOLD) {
      // Scrolling down — hide.
      this._hide()
    } else if (delta < -this.constructor.THRESHOLD) {
      // Scrolling up — reveal.
      this._show()
    }

    this.lastPos = pos
  }

  _hide() {
    if (this.hidden) return
    this.hidden = true
    this.element.classList.add("translate-y-full")
  }

  _show() {
    if (!this.hidden) return
    this.hidden = false
    this.element.classList.remove("translate-y-full")
  }
}
