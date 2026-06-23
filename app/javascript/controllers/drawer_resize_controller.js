import { Controller } from "@hotwired/stimulus"

// Drag-to-resize for the bottom-right floating email drawer. Because the panel is
// anchored to the bottom-right corner, only the TOP edge (height) and LEFT edge
// (width) move; the top-left corner resizes both. Width and height are clamped to
// the viewport and persisted separately in localStorage. Below the `sm`
// breakpoint the drawer is a full-width bottom sheet, so the inline sizes are
// dropped (the responsive classes take over) and the handles are hidden via CSS.
// Modeled on panel_resize_controller, extended to two axes + inverted direction.
const SM = 640
const MARGIN = 32 // keep the panel clear of the viewport edges

export default class extends Controller {
  static values = {
    minWidth: { type: Number, default: 340 },
    maxWidth: { type: Number, default: 760 },
    minHeight: { type: Number, default: 360 },
    maxHeight: { type: Number, default: 940 },
    widthKey: { type: String, default: "campbooks:email-drawer-width" },
    heightKey: { type: String, default: "campbooks:email-drawer-height" }
  }

  connect() {
    this.boundMove = this.#move.bind(this)
    this.boundStop = this.#stop.bind(this)
    this.boundApply = this.#apply.bind(this)

    const w = parseInt(localStorage.getItem(this.widthKeyValue), 10)
    const h = parseInt(localStorage.getItem(this.heightKeyValue), 10)
    this.savedWidth = Number.isFinite(w) ? w : null
    this.savedHeight = Number.isFinite(h) ? h : null

    window.addEventListener("resize", this.boundApply)
    this.#apply()
  }

  disconnect() {
    window.removeEventListener("resize", this.boundApply)
    this.#cleanup()
  }

  start(e) {
    if (!this.#isDesktop()) return
    e.preventDefault()
    e.stopPropagation()
    this.axis = e.params.axis || "xy"
    this.dragging = true
    this.startX = e.clientX
    this.startY = e.clientY
    const r = this.element.getBoundingClientRect()
    this.startWidth = r.width
    this.startHeight = r.height
    this.element.classList.add("select-none")
    this.element.style.transition = "none"
    document.addEventListener("mousemove", this.boundMove)
    document.addEventListener("mouseup", this.boundStop)
  }

  #move(e) {
    if (!this.dragging) return
    // Anchored bottom-right: dragging the handle left/up grows the panel.
    if (this.axis.includes("x")) {
      const w = this.#clamp(this.startWidth + (this.startX - e.clientX), this.minWidthValue, this.#maxW())
      this.element.style.width = `${w}px`
    }
    if (this.axis.includes("y")) {
      const h = this.#clamp(this.startHeight + (this.startY - e.clientY), this.minHeightValue, this.#maxH())
      this.element.style.height = `${h}px`
    }
  }

  #stop() {
    if (!this.dragging) return
    this.dragging = false
    this.element.classList.remove("select-none")
    this.element.style.transition = ""
    const r = this.element.getBoundingClientRect()
    if (this.axis.includes("x")) {
      this.savedWidth = Math.round(r.width)
      localStorage.setItem(this.widthKeyValue, this.savedWidth)
    }
    if (this.axis.includes("y")) {
      this.savedHeight = Math.round(r.height)
      localStorage.setItem(this.heightKeyValue, this.savedHeight)
    }
    document.removeEventListener("mousemove", this.boundMove)
    document.removeEventListener("mouseup", this.boundStop)
  }

  // Restore the saved size on desktop; drop inline sizes on mobile so the
  // bottom-sheet classes win. No-op mid-drag.
  #apply() {
    if (this.dragging) return
    if (!this.#isDesktop()) {
      this.element.style.width = ""
      this.element.style.height = ""
      return
    }
    if (this.savedWidth) this.element.style.width = `${this.#clamp(this.savedWidth, this.minWidthValue, this.#maxW())}px`
    if (this.savedHeight) this.element.style.height = `${this.#clamp(this.savedHeight, this.minHeightValue, this.#maxH())}px`
  }

  #clamp(v, min, max) { return Math.max(min, Math.min(max, v)) }
  #maxW() { return Math.min(this.maxWidthValue, window.innerWidth - MARGIN) }
  #maxH() { return Math.min(this.maxHeightValue, window.innerHeight - MARGIN) }
  #isDesktop() { return window.innerWidth >= SM }

  #cleanup() {
    document.removeEventListener("mousemove", this.boundMove)
    document.removeEventListener("mouseup", this.boundStop)
  }
}
