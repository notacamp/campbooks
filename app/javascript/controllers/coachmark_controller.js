import { Controller } from "@hotwired/stimulus"

// Positions a one-time coachmark popover + pulse highlight over an on-page target
// (which may be lazy-loaded), reveals it, keeps it pinned on scroll/resize, and on
// dismiss records the tour so it greets the user only once. No dimming backdrop —
// the target stays interactive, and tapping it dismisses the coachmark too.
const PAD = 6 // highlight padding around the target
const GAP = 12 // gap between target and bubble
const MARGIN = 12 // min viewport margin for the bubble
const GIVE_UP_MS = 10000

export default class extends Controller {
  static targets = ["highlight", "bubble", "caret"]
  static values = {
    anchor: String,
    placement: { type: String, default: "bottom" },
    tourKey: String,
    dismissUrl: String,
    // Highlight the union of the anchor's children (tight to e.g. rings inside a
    // wide scroller) instead of the anchor's own box. Off for single-element targets.
    unionChildren: { type: Boolean, default: false }
  }

  connect() {
    this.shown = false
    this.onReflow = this.reflow.bind(this)
    // The target (skim rings) is lazy — retry as turbo frames load / the DOM mutates.
    this.retry = () => this.tryShow()
    document.addEventListener("turbo:frame-load", this.retry)
    this.observer = new MutationObserver(this.retry)
    this.observer.observe(this.anchorEl || document.body, { childList: true, subtree: true })
    this.deadline = setTimeout(() => this.teardownWatchers(), GIVE_UP_MS)
    this.tryShow()
  }

  disconnect() {
    this.teardownWatchers()
    this.stopTracking()
    clearTimeout(this.deadline)
  }

  teardownWatchers() {
    if (this.retry) document.removeEventListener("turbo:frame-load", this.retry)
    if (this.observer) this.observer.disconnect()
    this.retry = null
    this.observer = null
  }

  // First *visible* match — lets one selector cover responsive twins (e.g. a
  // desktop pill and a mobile tab), the hidden one skipped via zero client rects.
  get anchorEl() {
    if (!this.anchorValue) return null
    const els = document.querySelectorAll(this.anchorValue)
    for (const el of els) if (el.getClientRects().length > 0) return el
    return els[0] || null
  }

  // The box to highlight: the union of the anchor's child rects when unionChildren
  // is set (tight to rings inside a wide scroller), else the anchor's own box.
  // Returns null when there's nothing meaningful to point at yet.
  targetRect() {
    const el = this.anchorEl
    if (!el) return null
    if (this.unionChildrenValue) {
      let box = null
      for (const child of el.children) {
        const r = child.getBoundingClientRect()
        if (r.width < 1 || r.height < 1) continue
        box = box
          ? { top: Math.min(box.top, r.top), left: Math.min(box.left, r.left), right: Math.max(box.right, r.right), bottom: Math.max(box.bottom, r.bottom) }
          : { top: r.top, left: r.left, right: r.right, bottom: r.bottom }
      }
      if (box) return box
    }
    const r = el.getBoundingClientRect()
    if (r.width < 1 || r.height < 1) return null
    return { top: r.top, left: r.left, right: r.right, bottom: r.bottom }
  }

  // Reveal once the target is real — skim frame loaded (not the skeleton) and the
  // rings have width.
  tryShow() {
    if (this.shown) return
    const frame = this.anchorEl?.querySelector("turbo-frame")
    if (frame && !frame.hasAttribute("complete")) return // still showing the skeleton
    const rect = this.targetRect()
    if (!rect || rect.right - rect.left < 16) return // nothing to skim yet
    this.shown = true
    this.teardownWatchers()
    clearTimeout(this.deadline)
    this.element.classList.remove("hidden")
    this.position()
    requestAnimationFrame(() => {
      this.position()
      this.highlightTarget.classList.remove("opacity-0")
      this.bubbleTarget.classList.remove("opacity-0")
    })
    this.startTracking()
  }

  startTracking() {
    window.addEventListener("scroll", this.onReflow, { capture: true, passive: true })
    window.addEventListener("resize", this.onReflow, { passive: true })
    // Tapping the target dismisses too — without blocking its own action.
    this.anchorClick = () => this.dismiss()
    this.anchorEl?.addEventListener("click", this.anchorClick)
  }

  stopTracking() {
    window.removeEventListener("scroll", this.onReflow, { capture: true })
    window.removeEventListener("resize", this.onReflow)
    if (this.anchorClick) this.anchorEl?.removeEventListener("click", this.anchorClick)
  }

  reflow() { if (this.shown) this.position() }

  position() {
    const rect = this.targetRect()
    if (!rect) return
    const vw = window.innerWidth
    const vh = window.innerHeight
    const w = rect.right - rect.left
    const h = rect.bottom - rect.top

    const hl = this.highlightTarget.style
    hl.left = `${rect.left - PAD}px`
    hl.top = `${rect.top - PAD}px`
    hl.width = `${w + PAD * 2}px`
    hl.height = `${h + PAD * 2}px`

    const bubble = this.bubbleTarget
    const bw = bubble.offsetWidth
    const bh = bubble.offsetHeight
    const above = this.placementValue === "top"
    let bx = Math.max(MARGIN, Math.min(rect.left, vw - bw - MARGIN))
    let by = above ? rect.top - GAP - bh - PAD : rect.bottom + GAP + PAD
    by = Math.max(MARGIN, Math.min(by, vh - bh - MARGIN))
    bubble.style.left = `${bx}px`
    bubble.style.top = `${by}px`

    // Caret points at the target's horizontal centre, clamped within the bubble.
    const cx = (rect.left + rect.right) / 2
    this.caretTarget.style.left = `${Math.max(14, Math.min(cx - bx - 6, bw - 26))}px`
  }

  dismiss(event) {
    if (event) event.preventDefault()
    if (this.element.classList.contains("hidden")) return
    this.element.classList.add("hidden")
    this.stopTracking()
    if (this.dismissUrlValue) {
      fetch(this.dismissUrlValue, {
        method: "POST",
        headers: { "X-CSRF-Token": this.csrf, "Accept": "application/json" }
      }).catch(() => {})
    }
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }
}
