import { Controller } from "@hotwired/stimulus"

// Drag-to-reschedule on the Day/Week time grids. The controller sits on the
// events track; each writable event box is a target. Pointer-drag moves the box
// (vertical = time, horizontal = day on the week grid), snaps to 15 minutes, then
// PATCHes /calendar_events/:id/reschedule (server enforces write permission).
// Reverts on failure; a plain click still opens the event.
export default class extends Controller {
  static targets = ["event"]
  static values = { hourPx: Number, startHour: Number, startDate: String, days: Number }

  eventTargetConnected(el) {
    el.style.touchAction = "none"
    el.style.cursor = "grab"
    // Event boxes are <a href>, which are natively draggable: without opting out,
    // Chrome starts its own link drag-and-drop on mousedown+move, firing
    // pointercancel and tearing down our pointer capture — so the reschedule
    // gesture never runs (the box just freezes a few px in). Disable native DnD.
    el.draggable = false
    el.addEventListener("dragstart", (e) => e.preventDefault())
    el.addEventListener("pointerdown", (e) => this._down(e, el))
    el.addEventListener("click", (e) => {
      if (el.dataset.dragged) { e.preventDefault(); e.stopPropagation(); delete el.dataset.dragged }
    })
  }

  _down(e, el) {
    if (e.button !== 0) return
    const track = this.element.getBoundingClientRect()
    const box = el.getBoundingClientRect()
    this.drag = {
      el, track,
      startX: e.clientX,
      startY: e.clientY,
      grabY: e.clientY - box.top,
      heightPx: box.height,
      durationMin: Math.max(15, Math.round((box.height / this.hourPxValue) * 60)),
      moved: false,
      topPx: parseFloat(el.style.top) || 0,
      dayIndex: 0,
      orig: { top: el.style.top, left: el.style.left, width: el.style.width }
    }
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
    // Ignore sub-threshold jitter so a plain click still opens the event instead
    // of being swallowed as a zero-distance drag.
    if (!d.moved) {
      if (Math.abs(e.clientX - d.startX) < 4 && Math.abs(e.clientY - d.startY) < 4) return
      d.moved = true
    }
    e.preventDefault()

    let topPx = e.clientY - d.track.top - d.grabY
    topPx = Math.max(0, Math.min(topPx, d.track.height - d.heightPx))
    d.topPx = topPx
    d.el.style.top = `${topPx}px`

    if (this.daysValue > 1) {
      const colW = d.track.width / this.daysValue
      let idx = Math.floor((e.clientX - d.track.left) / colW)
      idx = Math.max(0, Math.min(idx, this.daysValue - 1))
      d.dayIndex = idx
      const dayW = 100 / this.daysValue
      d.el.style.left = `${(idx * dayW).toFixed(3)}%`
      d.el.style.width = `${(dayW - 0.3).toFixed(3)}%`
    }
    d.el.style.opacity = "0.8"
    d.el.style.zIndex = "40"
  }

  async _up() {
    const d = this.drag
    this.drag = null
    if (!d) return
    this._teardown(d)
    if (!d.moved) return
    d.el.dataset.dragged = "1"

    const startMin = Math.round((d.topPx / this.hourPxValue * 60) / 15) * 15 + this.startHourValue * 60
    const start = this._iso(d.dayIndex, startMin)
    const end = this._iso(d.dayIndex, startMin + d.durationMin)

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
      this._revert(d)
    }
  }

  // A native gesture takeover (e.g. the browser claiming the pointer for a scroll)
  // fires pointercancel; abandon the drag and put the box back rather than leaving
  // it stranded mid-track.
  _cancel() {
    const d = this.drag
    this.drag = null
    if (!d) return
    this._teardown(d)
    if (d.moved) this._revert(d)
  }

  _teardown(d) {
    d.el.removeEventListener("pointermove", this.onMove)
    d.el.removeEventListener("pointerup", this.onUp)
    d.el.removeEventListener("pointercancel", this.onCancel)
    d.el.style.opacity = ""
    d.el.style.zIndex = ""
  }

  _revert(d) {
    d.el.style.top = d.orig.top
    d.el.style.left = d.orig.left
    d.el.style.width = d.orig.width
  }

  // Local wall-clock ISO (no zone) for the dropped slot; the server parses it in
  // the app time zone, matching how the grid positions events.
  _iso(dayIndex, minutes) {
    const dt = new Date(`${this.startDateValue}T00:00:00`)
    dt.setDate(dt.getDate() + dayIndex)
    dt.setHours(0, minutes, 0, 0)
    const p = (x) => String(x).padStart(2, "0")
    return `${dt.getFullYear()}-${p(dt.getMonth() + 1)}-${p(dt.getDate())}T${p(dt.getHours())}:${p(dt.getMinutes())}`
  }

  _token() {
    return document.querySelector('meta[name="csrf-token"]')?.content
  }
}
