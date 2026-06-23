import { Controller } from "@hotwired/stimulus"

// Scroll-focus for the home feed: marks the entry nearest the viewport centre
// with data-focused, so the eye settles on one card at a time. That flag is also
// the cursor for feed-keyboard (acts on the focused card) and the anchor for its
// on-card shortcut chips, so the controller tracks focus on every device —
// including under prefers-reduced-motion, where only the *dimming* is dropped
// (the dim CSS is gated on (prefers-reduced-motion: no-preference); the flag is
// not). Progressive enhancement: the dimming only bites once this controller
// flags the list with data-feed-focus, so with no JS every card stays full
// strength. Cooperates with feed-keyboard: j/k scroll the active card to centre,
// so it becomes the nearest and sharpens.
//
// An IntersectionObserver keeps a small set of on-screen units; each animation
// frame only scans those for the one closest to centre, so cost stays flat
// however far the infinite feed grows. A MutationObserver picks up turbo-
// appended pages and removed cards.
export default class extends Controller {
  connect() {
    this.visible = new Set()
    this.focused = null
    this.ticking = false
    this.onScroll = () => this.schedule()

    this.io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) this.visible.add(e.target)
        else this.visible.delete(e.target)
      }
      this.schedule()
    }, { rootMargin: "-10% 0px -10% 0px" })

    this.observeUnits()

    this.mo = new MutationObserver(() => this.observeUnits())
    this.mo.observe(this.element, { childList: true, subtree: true })

    window.addEventListener("scroll", this.onScroll, { passive: true })
    window.addEventListener("resize", this.onScroll, { passive: true })

    this.update()                                    // mark the focus before…
    this.element.setAttribute("data-feed-focus", "") // …dimming turns on (no flash)
  }

  disconnect() {
    this.io?.disconnect()
    this.mo?.disconnect()
    window.removeEventListener("scroll", this.onScroll)
    window.removeEventListener("resize", this.onScroll)
    this.element.removeAttribute("data-feed-focus")
  }

  units() {
    return this.element.querySelectorAll("[data-feed-focus-unit]")
  }

  observeUnits() {
    this.units().forEach((el) => this.io.observe(el)) // re-observing is a no-op
    this.schedule()
  }

  schedule() {
    if (this.ticking) return
    this.ticking = true
    requestAnimationFrame(() => { this.ticking = false; this.update() })
  }

  update() {
    // Before the observer first reports, fall back to a full scan.
    const pool = this.visible.size ? this.visible : new Set(this.units())
    if (pool.size === 0) return

    // At a scroll boundary the centre line can't reach the edge card, so the raw
    // midpoint pick would skip a short first/last card and settle on its taller
    // neighbour — the first-load and scroll-back-to-top symptom. Let reading
    // order, not screen position, win there.
    let best = this.edgeUnit()

    if (!best) {
      const mid = window.innerHeight / 2
      let bestDist = Infinity
      for (const el of pool) {
        if (!el.isConnected) { this.visible.delete(el); continue }
        const r = el.getBoundingClientRect()
        // Distance from the viewport centre to the card's nearest edge — 0 while
        // the centre line is inside the card, so a tall card stays focused across it.
        const dist = r.top > mid ? r.top - mid : (r.bottom < mid ? mid - r.bottom : 0)
        if (dist < bestDist) { bestDist = dist; best = el }
      }
    }
    if (!best || best === this.focused) return

    this.focused?.removeAttribute("data-focused")
    best.setAttribute("data-focused", "")
    this.focused = best
  }

  // When the window is pinned at its top (first load, or after scrolling back up)
  // or bottom, the centre line physically can't reach a short edge card, so the
  // midpoint heuristic wrongly settles on the neighbour. Return that edge card to
  // pin it by reading order instead. null when we're mid-scroll (use the midpoint
  // pick) or the edge card isn't actually in view — e.g. it sits below the fold
  // under a tall header at scrollY 0, where forcing it would be wrong.
  edgeUnit() {
    const atTop = window.scrollY <= 2
    const atBottom = window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2
    if (!atTop && !atBottom) return null

    const units = this.units()
    if (units.length === 0) return null
    const unit = atTop ? units[0] : units[units.length - 1]

    const r = unit.getBoundingClientRect()
    const onScreen = r.bottom > 0 && r.top < window.innerHeight
    return onScreen ? unit : null
  }
}
