import { Controller } from "@hotwired/stimulus"

// Click/tap or drag an empty slot to create an event. Month mode: a click on a day
// cell opens the new-event form for that day. Time-grid mode (day/week):
//   - mouse: drag paints a time-range selection; a plain click uses 1h.
//   - touch: stay passive so the tall grid still scrolls — a TAP creates (1h), a
//     drag scrolls (no selection box). pointercancel (the browser taking over for a
//     scroll) is handled so selection boxes never pile up.
// Coexists with calendar-dnd; ignores pointerdowns that land on an existing event.
export default class extends Controller {
  static values = { mode: String, hourPx: Number, startHour: Number, startDate: String, days: Number }

  connect() {
    if (this.modeValue === "month") {
      this._onClick = (e) => this._monthClick(e)
      this.element.addEventListener("click", this._onClick)
    } else {
      this._onDown = (e) => this._gridDown(e)
      this.element.addEventListener("pointerdown", this._onDown)
      // A live "click here to add" hint that tracks the cursor over empty time,
      // so an empty grid reads as tappable rather than inert. Mouse only — touch
      // has no hover and needs the surface free to scroll.
      this._onHover = (e) => this._hoverMove(e)
      this._onHoverLeave = () => this._hoverHide()
      this.element.addEventListener("pointermove", this._onHover)
      this.element.addEventListener("pointerleave", this._onHoverLeave)
    }
  }

  disconnect() {
    if (this._onClick) this.element.removeEventListener("click", this._onClick)
    if (this._onDown) this.element.removeEventListener("pointerdown", this._onDown)
    if (this._onHover) this.element.removeEventListener("pointermove", this._onHover)
    if (this._onHoverLeave) this.element.removeEventListener("pointerleave", this._onHoverLeave)
    if (this.ghost) { this.ghost.remove(); this.ghost = null }
    this._cleanup()
  }

  _onEvent(target) {
    return target.closest('a[href*="/calendar_events/"]')
  }

  _monthClick(e) {
    if (this._onEvent(e.target)) return
    const cell = e.target.closest("[data-new-url]")
    if (cell) this._openModal(cell.dataset.newUrl)
  }

  _gridDown(e) {
    if (e.button !== 0 || this._onEvent(e.target)) return
    this._cleanup() // never leave a prior gesture's listeners/box around
    const track = this.element.getBoundingClientRect()
    this.sel = {
      track,
      tapOnly: e.pointerType !== "mouse", // touch/pen: tap to create, drag to scroll
      startClientX: e.clientX,
      startClientY: e.clientY,
      startY: e.clientY - track.top,
      endY: e.clientY - track.top,
      dayIndex: this._dayAt(e.clientX, track),
      moved: false,
      aborted: false
    }
    this._onMove = (ev) => this._gridMove(ev)
    this._onUp = () => this._gridUp()
    this._onCancel = () => this._cleanup()
    window.addEventListener("pointermove", this._onMove, { passive: this.sel.tapOnly })
    window.addEventListener("pointerup", this._onUp)
    window.addEventListener("pointercancel", this._onCancel)
  }

  _gridMove(e) {
    const s = this.sel
    if (!s) return
    if (s.tapOnly) {
      // Movement on touch = scroll intent: bow out and let the page move.
      if (Math.abs(e.clientX - s.startClientX) > 10 || Math.abs(e.clientY - s.startClientY) > 10) {
        s.aborted = true
        this._cleanup()
      }
      return
    }
    e.preventDefault()
    s.moved = true
    s.endY = e.clientY - s.track.top
    if (!this.box) this._createBox()
    this._paint()
  }

  _gridUp() {
    const s = this.sel
    this._cleanup()
    if (!s || s.aborted) return
    const y1 = Math.min(s.startY, s.endY)
    const y2 = s.moved ? Math.max(s.startY, s.endY) : s.startY
    const startMin = this._minAt(y1)
    let endMin = s.moved ? this._minAt(y2) : startMin + 60
    if (endMin <= startMin) endMin = startMin + 30
    const start = this._iso(s.dayIndex, startMin)
    const end = this._iso(s.dayIndex, endMin)
    this._openModal(`/calendar_events/new?start=${encodeURIComponent(start)}&end=${encodeURIComponent(end)}`)
  }

  // Open the create/edit modal instead of navigating — keeps you on the calendar.
  // The calendar-event-modal controller (mounted on the calendar) listens for this.
  _openModal(url) {
    window.dispatchEvent(new CustomEvent("calendar-event-modal:open", { detail: { url } }))
  }

  _cleanup() {
    if (this._onMove) window.removeEventListener("pointermove", this._onMove)
    if (this._onUp) window.removeEventListener("pointerup", this._onUp)
    if (this._onCancel) window.removeEventListener("pointercancel", this._onCancel)
    this._onMove = this._onUp = this._onCancel = null
    if (this.box) { this.box.remove(); this.box = null }
    this.sel = null
  }

  _createBox() {
    this.box = document.createElement("div")
    this.box.style.cssText =
      "position:absolute;background:rgba(120,120,120,0.18);border:1px solid rgba(120,120,120,0.45);border-radius:6px;pointer-events:none;z-index:20"
    this.element.appendChild(this.box)
  }

  // ── Hover affordance (mouse, time-grid) ────────────────────────────────────
  _hoverMove(e) {
    if (e.pointerType !== "mouse") return // touch scrolls; no hover cue
    if (this.sel || this.box) return this._hoverHide() // an active drag owns the surface
    if (this._onEvent(e.target)) return this._hoverHide() // over an existing event
    const track = this.element.getBoundingClientRect()
    const startMin = this._snapHover(e.clientY - track.top)
    const dayIndex = this._dayAt(e.clientX, track)
    const dayW = 100 / this.daysValue
    const g = this._ensureGhost()
    g.style.top = `${((startMin - this.startHourValue * 60) / 60) * this.hourPxValue}px`
    g.style.height = `${this.hourPxValue}px`
    g.style.left = `${(dayIndex * dayW).toFixed(3)}%`
    g.style.width = `${(dayW - 0.3).toFixed(3)}%`
    g.style.opacity = "1"
  }

  _hoverHide() {
    if (this.ghost) this.ghost.style.opacity = "0"
  }

  _ensureGhost() {
    if (this.ghost) return this.ghost
    const g = document.createElement("div")
    g.style.cssText =
      "position:absolute;z-index:15;pointer-events:none;border-radius:8px;opacity:0;" +
      "background:color-mix(in oklch, var(--muted-foreground) 10%, transparent);" +
      "border:1px dashed color-mix(in oklch, var(--muted-foreground) 45%, transparent);" +
      "display:flex;align-items:center;justify-content:center;transition:opacity .12s ease-out;"
    g.innerHTML =
      '<span style="font-size:15px;line-height:1;font-weight:600;color:var(--muted-foreground);opacity:.75">+</span>'
    this.element.appendChild(g)
    this.ghost = g
    return g
  }

  _snapHover(y) {
    const raw = (y / this.hourPxValue) * 60 + this.startHourValue * 60
    return Math.max(this.startHourValue * 60, Math.round(raw / 30) * 30)
  }

  _dayAt(clientX, track) {
    if (this.daysValue <= 1) return 0
    const colW = track.width / this.daysValue
    return Math.max(0, Math.min(Math.floor((clientX - track.left) / colW), this.daysValue - 1))
  }

  _paint() {
    const s = this.sel
    if (!this.box || !s) return
    const top = Math.min(s.startY, s.endY)
    const height = Math.max(6, Math.abs(s.endY - s.startY))
    const dayW = 100 / this.daysValue
    this.box.style.top = `${top}px`
    this.box.style.height = `${height}px`
    this.box.style.left = `${(s.dayIndex * dayW).toFixed(3)}%`
    this.box.style.width = `${(dayW - 0.3).toFixed(3)}%`
  }

  _minAt(y) {
    const raw = (y / this.hourPxValue) * 60 + this.startHourValue * 60
    return Math.max(0, Math.round(raw / 15) * 15)
  }

  _iso(dayIndex, minutes) {
    const dt = new Date(`${this.startDateValue}T00:00:00`)
    dt.setDate(dt.getDate() + dayIndex)
    dt.setHours(0, minutes, 0, 0)
    const p = (x) => String(x).padStart(2, "0")
    return `${dt.getFullYear()}-${p(dt.getMonth() + 1)}-${p(dt.getDate())}T${p(dt.getHours())}:${p(dt.getMinutes())}`
  }
}
