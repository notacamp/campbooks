import { Controller } from "@hotwired/stimulus"

// Auto-hides this element when the user scrolls down and reveals it on
// scroll-up. Designed for the mobile bottom nav (#bottom-nav): the bar slides
// off-screen to reclaim vertical space during downward reading, then glides
// back as soon as the user scrolls up to act or navigate.
//
// Technique: passive window scroll listener + requestAnimationFrame throttle
// (the same pattern used by feed-focus). A small threshold (10 px) avoids
// jitter from micro-bounces or inertia tails. An additional guard keeps the
// bar shown while near the top of the page.
export default class extends Controller {
  // How many pixels the user must scroll in one direction before we react —
  // filters out micro-jitter and inertia tails.
  static THRESHOLD = 10

  // Scroll position below which the bar is always shown regardless of direction,
  // so it doesn't disappear on the very first tiny scroll.
  static TOP_GUARD = 60

  connect() {
    this.lastScrollY = window.scrollY
    this.ticking = false
    this.hidden = false

    this.element.classList.add("transition-transform", "duration-300")

    this.onScroll = () => {
      if (this.ticking) return
      this.ticking = true
      requestAnimationFrame(() => {
        this.ticking = false
        this._update()
      })
    }

    window.addEventListener("scroll", this.onScroll, { passive: true })
  }

  disconnect() {
    window.removeEventListener("scroll", this.onScroll)
  }

  _update() {
    const scrollY = window.scrollY
    const delta = scrollY - this.lastScrollY

    if (scrollY <= this.constructor.TOP_GUARD) {
      // Always show near the top of the page.
      this._show()
    } else if (delta > this.constructor.THRESHOLD) {
      // Scrolling down — hide.
      this._hide()
    } else if (delta < -this.constructor.THRESHOLD) {
      // Scrolling up — reveal.
      this._show()
    }

    this.lastScrollY = scrollY
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
