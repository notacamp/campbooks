import { Controller } from "@hotwired/stimulus"
import { buildSegments } from "controllers/skim_utils"

// Contact Skim — new-sender triage in the Skim "stories" idiom, mirroring
// skim_mode_controller so it reads as "every other skim function" rather than a
// Tinder deck. Segmented progress sits on top; one card shows at a time and
// cross-fades to the next. → / tap-right / swipe-left skips ahead (leaves the
// sender pending); ← / tap-left / swipe-right steps back. Allow (A) and Block (B)
// POST the current card's decision endpoint (set_state, JSON) and advance.
// Swipe only navigates, never decides — that's the buttons and keys, exactly like
// the email and document skims.
export default class extends Controller {
  static targets = ["frame", "segments", "progress", "done", "close"]
  static values = { index: { type: Number, default: 0 } }

  connect() {
    this.touch = null
    this.render()
    // Focus the surface so keyboard shortcuts work without a click first.
    this.element.focus({ preventScroll: true })
  }

  // ---- input ---------------------------------------------------------------

  onKeydown(event) {
    const tag = (event.target.tagName || "").toLowerCase()
    if (tag === "input" || tag === "textarea" || tag === "select" || event.target.isContentEditable) return

    switch (event.key) {
      case "ArrowRight": this.next(); break
      case "ArrowLeft":  this.prev(); break
      case "a": case "A": this.allow(); break
      case "b": case "B": this.block(); break
      case "Escape": this.closeTarget?.click(); return
      default: return
    }
    event.preventDefault()
  }

  // Card action buttons (data-skim-action) bubble up to here.
  onClick(event) {
    const button = event.target.closest("[data-skim-action]")
    if (!button) return
    switch (button.dataset.skimAction) {
      case "allow": this.allow(); break
      case "block": this.block(); break
      case "skip":  this.next(); break
    }
  }

  onTouchStart(event) {
    const t = event.changedTouches[0]
    this.touch = { x: t.clientX, y: t.clientY }
  }

  onTouchEnd(event) {
    if (!this.touch) return
    const t = event.changedTouches[0]
    const dx = t.clientX - this.touch.x
    const dy = t.clientY - this.touch.y
    this.touch = null
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) {
      if (dx < 0) this.next() // swipe left → next (skip)
      else this.prev()        // swipe right → back
    }
  }

  // ---- navigation ----------------------------------------------------------

  next() { this.advance() } // skip: leave the sender pending, advance
  advance() { if (this.indexValue < this.frameTargets.length) this.indexValue += 1 }
  prev() { if (this.indexValue > 0) this.indexValue -= 1 }

  // ---- decisions -----------------------------------------------------------

  allow() { this.decide("approveUrl") }
  block() { this.decide("blockUrl") }

  decide(urlKey) {
    const frame = this.currentFrame()
    if (!frame) return
    this.post(frame.dataset[urlKey])
    this.advance()
  }

  currentFrame() { return this.frameTargets[this.indexValue] || null }

  // Fire-and-forget: the optimistic cross-fade is the feedback.
  post(url) {
    if (!url) return
    fetch(url, {
      method: "POST",
      headers: { Accept: "application/json", "X-CSRF-Token": this.csrf },
      keepalive: true
    }).catch(() => {})
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }

  // ---- render --------------------------------------------------------------

  get isDone() { return this.frameTargets.length === 0 || this.indexValue >= this.frameTargets.length }

  indexValueChanged() { this.render() }

  render() {
    this.layoutFrames()
    if (this.hasDoneTarget) {
      this.doneTarget.classList.toggle("hidden", !this.isDone)
      this.doneTarget.classList.toggle("flex", this.isDone)
    }
    const total = this.frameTargets.length
    const pos = this.isDone ? total + 1 : this.indexValue + 1
    if (this.hasSegmentsTarget) buildSegments(this.segmentsTarget, total, pos, 20)
    this.updateProgress()
  }

  layoutFrames() {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const trans = reduce ? "" : "transition:opacity .28s ease;"
    this.frameTargets.forEach((f, i) => {
      if (i === this.indexValue) {
        f.classList.remove("hidden")
        f.style.cssText = trans + "opacity:1;z-index:10;pointer-events:auto;"
      } else {
        f.classList.add("hidden")
        f.style.cssText = trans + "opacity:0;pointer-events:none;"
      }
    })
  }

  updateProgress() {
    if (!this.hasProgressTarget) return
    const done = Math.min(this.indexValue, this.frameTargets.length)
    const tpl = this.progressTarget.dataset.template || "__D__ / __T__"
    this.progressTarget.textContent = tpl.replace("__D__", done).replace("__T__", this.frameTargets.length)
  }
}
