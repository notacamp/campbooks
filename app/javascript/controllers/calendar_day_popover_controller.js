import { Controller } from "@hotwired/stimulus"

// A month-grid day only shows the first few events; the rest hide behind a
// "+N more" chip (and, on phones, the row of colored dots). Clicking either opens
// this popover — the full list of that day's events, reminders and scheduled mail,
// anchored to the cell.
//
// The panel lives inside the cell but is positioned `position: fixed`, so it
// escapes the month card's `overflow-hidden` (which otherwise clips anything past
// a cell edge) and the lg fixed-height cells. Only one popover is open at a time;
// a click outside, Escape, or any page scroll/resize closes it.
export default class extends Controller {
  static targets = ["panel", "trigger"]

  connect() {
    this._onDocDown = (e) => { if (!this._contains(e.target)) this.close() }
    this._onKey = (e) => { if (e.key === "Escape") this.close() }
    this._onScroll = (e) => { if (!this._panelHas(e.target)) this.close() }
    this._onResize = () => this.close()
    this._onPeerOpen = (e) => { if (e.detail !== this) this.close() }

    document.addEventListener("pointerdown", this._onDocDown, true)
    document.addEventListener("keydown", this._onKey)
    window.addEventListener("scroll", this._onScroll, true)
    window.addEventListener("resize", this._onResize)
    window.addEventListener("calendar-day-popover:open", this._onPeerOpen)
  }

  disconnect() {
    document.removeEventListener("pointerdown", this._onDocDown, true)
    document.removeEventListener("keydown", this._onKey)
    window.removeEventListener("scroll", this._onScroll, true)
    window.removeEventListener("resize", this._onResize)
    window.removeEventListener("calendar-day-popover:open", this._onPeerOpen)
  }

  toggle(event) {
    event.preventDefault()
    event.stopPropagation()
    this.open_ ? this.close() : this.open()
  }

  open() {
    if (!this.hasPanelTarget) return
    // Close any other open day popover first.
    window.dispatchEvent(new CustomEvent("calendar-day-popover:open", { detail: this }))
    this.panelTarget.classList.remove("hidden")
    this._position()
    this._setExpanded(true)
    this.open_ = true
  }

  close() {
    if (!this.hasPanelTarget || !this.open_) return
    this.panelTarget.classList.add("hidden")
    this._setExpanded(false)
    this.open_ = false
  }

  // Anchor the (fixed) panel to the cell's top-left, then clamp it inside the
  // viewport — flipping upward when there isn't room below.
  _position() {
    const panel = this.panelTarget
    const cell = this.element.getBoundingClientRect()
    const margin = 8
    const pw = panel.offsetWidth
    const ph = panel.offsetHeight

    let left = Math.min(cell.left, window.innerWidth - pw - margin)
    left = Math.max(margin, left)

    let top = cell.top
    if (top + ph + margin > window.innerHeight) top = cell.bottom - ph
    top = Math.min(top, window.innerHeight - ph - margin)
    top = Math.max(margin, top)

    panel.style.left = `${Math.round(left)}px`
    panel.style.top = `${Math.round(top)}px`
  }

  _setExpanded(on) {
    this.triggerTargets.forEach((t) => t.setAttribute("aria-expanded", on ? "true" : "false"))
  }

  _contains(node) {
    return this.element.contains(node) || this._panelHas(node)
  }

  _panelHas(node) {
    return this.hasPanelTarget && this.panelTarget.contains(node)
  }
}
