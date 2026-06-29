import { Controller } from "@hotwired/stimulus"
import { skimOverlayOpen } from "controllers/skim_utils"

// Keyboard control for the home feed, acting on the card the reader is looking at
// — the one feed-focus highlights as nearest the viewport centre ([data-focused]).
// There is no separate focus ring: the scroll-highlight IS the cursor, so j/k and
// the arrows just move which card is centred and everything acts on it.
//
//   j / ArrowDown   focus next card        k / ArrowUp   focus previous card
//   → / Enter / o   the card's primary      ←            dismiss / escape
//   e r c s …       the card's lettered actions (whatever buttons it shows)
//
// Every action is a real click on the card's own button (data-feed-primary /
// data-feed-dismiss / data-feed-key), so it reuses the exact Turbo path a tap
// takes — toast, Undo and removal animation included. Cards with no escape button
// (e.g. calendar) fall back to the feed-dismiss endpoint on the wrapper.
//
// Honors typing context, modifier keys, open dialogs. Units are re-queried each
// keypress, so turbo-stream-appended pages are picked up automatically. Mobile
// drives the same actions through swipe gestures (see feed_swipe / Swipeable), so
// this controller is a no-op without a keyboard.
export default class extends Controller {
  connect() {
    this.onKey = this.handleKey.bind(this)
    document.addEventListener("keydown", this.onKey)
  }

  disconnect() {
    document.removeEventListener("keydown", this.onKey)
  }

  handleKey(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    const target = event.target
    if (target instanceof Element && target.closest("input, textarea, select, [contenteditable='true']")) return
    if (document.querySelector("dialog[open]")) return
    // The Skim overlay is a role="dialog" panel, not a native <dialog open>, so
    // guard it explicitly — otherwise feed shortcuts also fire on the cards behind it.
    if (skimOverlayOpen()) return
    if (this.units().length === 0) return

    switch (event.key) {
      case "j": case "ArrowDown": this.moveFocus(1); event.preventDefault(); break
      case "k": case "ArrowUp":   this.moveFocus(-1); event.preventDefault(); break
      case "ArrowRight": case "Enter": case "o":
        if (this.click("[data-feed-primary]")) event.preventDefault(); break
      case "ArrowLeft":
        if (this.dismiss()) event.preventDefault(); break
      default:
        // A single printable letter → the card's matching lettered action.
        if (/^[a-z]$/.test(event.key) && this.click(`[data-feed-key="${event.key}"]`)) event.preventDefault()
    }
  }

  // All scroll-focus units (a normal card, or a whole tag-queue), in document order.
  units() {
    return Array.from(this.element.querySelectorAll("[data-feed-focus-unit]"))
  }

  // The card currently under the reader's eye. feed-focus keeps exactly one flagged;
  // before its first pass (or under odd timing) fall back to the first unit.
  active() {
    return this.element.querySelector("[data-feed-focus-unit][data-focused]") || this.units()[0] || null
  }

  // Move the highlight by stepping which unit sits at the viewport centre. We flag
  // the target immediately (responsive chips) and scroll it to centre; feed-focus
  // then settles on the same one, so the two never disagree once the scroll lands.
  moveFocus(delta) {
    const units = this.units()
    const current = this.active()
    const i = Math.max(0, units.indexOf(current))
    const next = units[Math.max(0, Math.min(units.length - 1, i + delta))]
    if (!next || next === current) return

    current?.removeAttribute("data-focused")
    next.setAttribute("data-focused", "")

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    next.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "center" })
  }

  // Click the first element matching `selector` inside the active unit. Returns
  // true if something was clicked (so the caller can preventDefault).
  click(selector) {
    const el = this.active()?.querySelector(selector)
    if (!el) return false
    el.click()
    return true
  }

  // ← : the card's own escape button if it has one (archive / dismiss / not-now),
  // else the wrapper's feed-dismiss endpoint (cards like calendar with no escape).
  dismiss() {
    if (this.click("[data-feed-dismiss]")) return true

    const url = this.active()?.getAttribute("data-feed-dismiss-url")
    if (!url) return false
    this.postDismiss(url)
    return true
  }

  postDismiss(url) {
    const token = document.querySelector('meta[name="csrf-token"]')?.content
    fetch(url, {
      method: "POST",
      headers: { "Accept": "text/vnd.turbo-stream.html", "X-CSRF-Token": token || "" },
      credentials: "same-origin"
    })
      .then((r) => (r.ok ? r.text() : null))
      .then((html) => { if (html && window.Turbo) window.Turbo.renderStreamMessage(html) })
      .catch(() => {})
  }
}
