import { Controller } from "@hotwired/stimulus"

// Drag-to-resize for a docked pane. The handle lives on one edge of the element;
// dragging it grows/shrinks the element's width, clamped to [minWidth, maxWidth]
// and persisted to localStorage. `edge` says which edge the handle sits on so the
// drag direction is correct: "left" for a right-docked panel (Discussion), "right"
// for a left-docked pane (the email thread list). When `minViewport` is set, the
// inline width is dropped below that viewport width so the element falls back to
// its responsive Tailwind classes (e.g. w-full) for single-pane mobile layouts.
export default class extends Controller {
  static targets = ["handle"]
  static values = {
    minWidth: { type: Number, default: 280 },
    maxWidth: { type: Number, default: 800 },
    // Desktop width to apply when nothing is saved yet. 0 means "leave the CSS
    // class width alone" (most panes); the inbox list sets it so its default
    // grows with the per-account filter.
    defaultWidth: { type: Number, default: 0 },
    minViewport: { type: Number, default: 0 },
    edge: { type: String, default: "left" },
    storageKey: { type: String, default: "campbooks:panel-width" }
  }

  connect() {
    this.boundMove = this.#move.bind(this)
    this.boundStop = this.#stop.bind(this)
    this.boundApply = this.#apply.bind(this)

    // Clamp any stored width to the current bounds — a value saved under an older
    // (smaller) min must not render the pane below today's floor.
    const saved = localStorage.getItem(this.storageKeyValue)
    const parsed = saved ? parseInt(saved, 10) : null
    this.savedWidth = parsed
      ? Math.max(this.minWidthValue, Math.min(this.maxWidthValue, parsed))
      : null

    if (this.minViewportValue > 0) window.addEventListener("resize", this.boundApply)
    this.#apply()
  }

  disconnect() {
    window.removeEventListener("resize", this.boundApply)
    this.#cleanup()
  }

  start(e) {
    if (!this.#isDesktop()) return
    e.preventDefault()
    this.dragging = true
    this.startX = e.clientX
    this.startWidth = this.element.getBoundingClientRect().width
    this.element.classList.add("select-none")
    this.element.style.transition = "none"
    document.addEventListener("mousemove", this.boundMove)
    document.addEventListener("mouseup", this.boundStop)
  }

  #move(e) {
    if (!this.dragging) return
    const delta = this.edgeValue === "right" ? e.clientX - this.startX : this.startX - e.clientX
    let width = this.startWidth + delta
    width = Math.max(this.minWidthValue, Math.min(this.maxWidthValue, width))
    this.element.style.width = `${width}px`
  }

  #stop() {
    if (!this.dragging) return
    this.dragging = false
    this.element.classList.remove("select-none")
    const width = Math.round(this.element.getBoundingClientRect().width)
    this.element.style.width = `${width}px`
    this.element.dataset.openWidth = width
    this.element.style.transition = ""
    this.savedWidth = width
    localStorage.setItem(this.storageKeyValue, width)
    document.removeEventListener("mousemove", this.boundMove)
    document.removeEventListener("mouseup", this.boundStop)
  }

  // Below the configured breakpoint, drop the inline width so the element's
  // responsive classes take over (single-pane mobile layout); otherwise restore
  // the saved width. No-op while dragging so a stray resize event can't fight it.
  #apply() {
    if (this.dragging) return
    // Collapsed (chat-panel rail) owns the width — don't impose the saved one.
    if (this.element.classList.contains("w-12")) return
    if (!this.#isDesktop()) {
      this.element.style.width = ""
      return
    }
    // Saved width wins; otherwise fall back to the configured default (0 = none,
    // keep the CSS class width). Clamp so neither can violate the current bounds.
    const target = this.savedWidth || this.defaultWidthValue
    if (target) {
      const width = Math.max(this.minWidthValue, Math.min(this.maxWidthValue, target))
      this.element.style.width = `${width}px`
      this.element.dataset.openWidth = width
    }
  }

  #isDesktop() {
    return this.minViewportValue === 0 || window.innerWidth >= this.minViewportValue
  }

  #cleanup() {
    document.removeEventListener("mousemove", this.boundMove)
    document.removeEventListener("mouseup", this.boundStop)
  }
}
