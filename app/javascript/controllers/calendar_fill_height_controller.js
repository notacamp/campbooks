import { Controller } from "@hotwired/stimulus"

// Grow a calendar surface to fill the space between its own top edge and the
// bottom of the viewport, so the month grid and the week/day time-grid use the
// whole page instead of floating in a short box. Height is only pinned on large
// screens (lg+); on mobile the surface keeps its natural, scrollable height.
//
// Measuring the element's live top (rather than a hard-coded calc) keeps it
// correct as the dismissible beta banner, topbar, or mobile browser chrome shift
// things around, and it re-fits on resize/orientation change.
//
// For the time-grid it doubles as the initial scroll: `scroll-to-hour` brings the
// working hours into view once on load (the grid itself spans all 24h).
export default class extends Controller {
  static values = {
    gap: { type: Number, default: 16 }, // breathing room left below the surface
    min: { type: Number, default: 480 }, // never collapse shorter than this (px)
    scrollToHour: Number,
    hourPx: Number,
    startHour: { type: Number, default: 0 }
  }

  connect() {
    this._fit = this._fit.bind(this)
    this._scrolled = false
    // Defer one frame so layout (and the sticky header above a scroll area) has
    // settled before we read getBoundingClientRect().
    requestAnimationFrame(this._fit)
    window.addEventListener("resize", this._fit)
    document.addEventListener("turbo:load", this._fit)
  }

  disconnect() {
    window.removeEventListener("resize", this._fit)
    document.removeEventListener("turbo:load", this._fit)
  }

  _fit() {
    if (window.matchMedia("(min-width: 1024px)").matches) {
      const top = this.element.getBoundingClientRect().top
      const height = Math.max(this.minValue, Math.round(window.innerHeight - top - this.gapValue))
      this.element.style.height = `${height}px`
    } else {
      this.element.style.height = "" // below lg, CSS (min/max-height) governs
    }

    // Bring the working hours into view once — even below lg, where a capped
    // scroll pane still opens on an empty pre-dawn grid otherwise.
    if (!this._scrolled && this.hasScrollToHourValue && this.hasHourPxValue) {
      this.element.scrollTop = Math.max(0, (this.scrollToHourValue - this.startHourValue) * this.hourPxValue)
      this._scrolled = true
    }
  }
}
