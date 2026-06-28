import { Controller } from "@hotwired/stimulus"

// Drag-to-reschedule ACROSS DAYS on the month grid. Each writable event chip is a
// draggable target; each day cell is a drop target. A pointer-drag floats a ghost
// clone of the chip, highlights the day cell under the pointer, and on release
// PATCHes /calendar_events/:id/reschedule with the event shifted by the whole-day
// delta (time-of-day and duration preserved). A plain click still opens the event.
//
// Pointer Events (not the HTML5 drag-and-drop API), mirroring calendar_dnd
// (the time-grid sibling): works on touch and dodges Chrome's native link-drag,
// which would otherwise hijack the gesture on these <a> chips.
export default class extends Controller {
  static targets = ["event", "day"]

  eventTargetConnected(el) {
    el.style.touchAction = "none"
    el.style.cursor = "grab"
    // <a href> chips are natively draggable; opt out so Chrome doesn't start its
    // own link DnD on mousedown+move (which fires pointercancel and kills capture).
    el.draggable = false
    el.addEventListener("dragstart", (e) => e.preventDefault())
    el.addEventListener("pointerdown", (e) => this._down(e, el))
    el.addEventListener("click", (e) => {
      if (el.dataset.dragged) { e.preventDefault(); e.stopPropagation(); delete el.dataset.dragged }
    })
  }

  _down(e, el) {
    if (e.button !== 0) return
    this.drag = { el, startX: e.clientX, startY: e.clientY, moved: false, ghost: null, overCell: null }
    el.setPointerCapture(e.pointerId)
    this.onMove = (ev) => this._move(ev)
    this.onUp = (ev) => this._up(ev)
    this.onCancel = () => this._cancel()
    el.addEventListener("pointermove", this.onMove)
    el.addEventListener("pointerup", this.onUp)
    el.addEventListener("pointercancel", this.onCancel)
  }

  _move(e) {
    const d = this.drag
    if (!d) return
    // Ignore sub-threshold jitter so a plain click still opens the event.
    if (!d.moved) {
      if (Math.abs(e.clientX - d.startX) < 4 && Math.abs(e.clientY - d.startY) < 4) return
      d.moved = true
      d.ghost = this._makeGhost(d.el)
    }
    e.preventDefault()
    d.ghost.style.left = `${e.clientX + 8}px`
    d.ghost.style.top = `${e.clientY + 8}px`

    const cell = this._cellAt(e.clientX, e.clientY)
    if (cell !== d.overCell) {
      this._highlight(d.overCell, false)
      this._highlight(cell, true)
      d.overCell = cell
    }
  }

  async _up(e) {
    const d = this.drag
    this.drag = null
    if (!d) return
    this._teardown(d)
    if (!d.moved) return
    d.el.dataset.dragged = "1"

    const cell = d.overCell || this._cellAt(e.clientX, e.clientY)
    this._highlight(cell, false)
    this._removeGhost(d)
    if (!cell) return

    const dayDelta = this._dayDelta(d.el.dataset.startAt, cell.dataset.date)
    if (dayDelta === 0) return

    const start = this._shift(d.el.dataset.startAt, dayDelta)
    const end = this._shift(d.el.dataset.endAt, dayDelta)
    try {
      const res = await fetch(`/calendar_events/${d.el.dataset.eventId}/reschedule`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json", "X-CSRF-Token": this._token() },
        body: JSON.stringify({ start_at: start, end_at: end })
      })
      if (!res.ok) throw new Error(res.status)
      if (window.Turbo) window.Turbo.visit(window.location.href, { action: "replace" })
      else window.location.reload()
    } catch {
      // The original chip never moved (we only floated a ghost), so a denied or
      // failed reschedule needs no revert — the grid is already correct.
    }
  }

  // A native gesture takeover (e.g. a scroll claiming the pointer) fires
  // pointercancel; clean up the ghost/highlight and abandon the drag.
  _cancel() {
    const d = this.drag
    this.drag = null
    if (!d) return
    this._teardown(d)
    this._highlight(d.overCell, false)
    this._removeGhost(d)
  }

  _teardown(d) {
    d.el.removeEventListener("pointermove", this.onMove)
    d.el.removeEventListener("pointerup", this.onUp)
    d.el.removeEventListener("pointercancel", this.onCancel)
  }

  _makeGhost(el) {
    const g = el.cloneNode(true)
    const r = el.getBoundingClientRect()
    Object.assign(g.style, {
      position: "fixed", left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`,
      pointerEvents: "none", opacity: "0.85", zIndex: "50", margin: "0",
      boxShadow: "0 4px 12px rgba(0,0,0,0.25)"
    })
    document.body.appendChild(g)
    return g
  }

  _removeGhost(d) {
    if (d.ghost) { d.ghost.remove(); d.ghost = null }
  }

  // The day cell under the pointer. The ghost has pointer-events:none so
  // elementFromPoint sees through it to the real cell (or a chip within it).
  _cellAt(x, y) {
    const el = document.elementFromPoint(x, y)
    return el ? el.closest('[data-calendar-month-dnd-target="day"]') : null
  }

  _highlight(cell, on) {
    if (!cell) return
    cell.classList.toggle("ring-2", on)
    cell.classList.toggle("ring-inset", on)
    cell.classList.toggle("ring-primary", on)
  }

  // Whole-day difference between an event's start date and a target date. Both are
  // bare YYYY-MM-DD compared at UTC midnight, so neither DST nor the browser's zone
  // can shift the count.
  _dayDelta(startWallClock, targetDate) {
    return this._daysBetween(startWallClock.slice(0, 10), targetDate)
  }

  _daysBetween(a, b) {
    const [ay, am, ad] = a.split("-").map(Number)
    const [by, bm, bd] = b.split("-").map(Number)
    return Math.round((Date.UTC(by, bm - 1, bd) - Date.UTC(ay, am - 1, ad)) / 86400000)
  }

  // Shift a "YYYY-MM-DDTHH:MM" wall-clock by N days, keeping the time-of-day exactly.
  // Pure UTC date math (never `new Date(iso)`), so the browser's zone never leaks in;
  // the server re-parses the same wall-clock string in the app zone.
  _shift(wallClock, days) {
    const [date, time] = wallClock.split("T")
    const [y, m, d] = date.split("-").map(Number)
    const shifted = new Date(Date.UTC(y, m - 1, d + days))
    const p = (x) => String(x).padStart(2, "0")
    const newDate = `${shifted.getUTCFullYear()}-${p(shifted.getUTCMonth() + 1)}-${p(shifted.getUTCDate())}`
    return time ? `${newDate}T${time}` : newDate
  }

  _token() {
    return document.querySelector('meta[name="csrf-token"]')?.content
  }
}
