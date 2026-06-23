import { Controller } from "@hotwired/stimulus"

// Opens Skim Mode as a full-screen, Instagram-stories-style overlay. The viewer
// is loaded into a turbo-frame the first time the overlay opens and re-fetched on
// every subsequent open so it reflects the current inbox (e.g. after archiving).
// "Skim all" opens at the first ring; a tray ring deep-links to its category.
// Escape, the in-viewer close button, or the backdrop dismiss it.
export default class extends Controller {
  static targets = ["panel", "frame"]

  open(event) {
    if (event) event.preventDefault()
    this.reveal()
    this.loadFrame(this.hasFrameTarget ? this.frameTarget.dataset.skimSrc : null)
  }

  // Deep-link open from a tray ring: start the viewer on a given theme.
  openTo(event) {
    if (event) event.preventDefault()
    this.reveal()
    const base = this.hasFrameTarget ? this.frameTarget.dataset.skimSrc : null
    const theme = event && event.params ? event.params.theme : null
    this.loadFrame(theme && base ? `${base}?start=${encodeURIComponent(theme)}` : base)
  }

  reveal() {
    this.panelTarget.classList.remove("hidden")
    document.documentElement.classList.add("overflow-hidden")
    this.focusStack()
  }

  loadFrame(url) {
    if (!this.hasFrameTarget || !url) return
    if (this.frameTarget.getAttribute("src") === url) this.frameTarget.reload()
    else this.frameTarget.setAttribute("src", url)
  }

  close() {
    this.panelTarget.classList.add("hidden")
    document.documentElement.classList.remove("overflow-hidden")
    this.refreshTray()
    // Let the "Skim all" orchestrator (home) disarm any pending document hand-off,
    // so a manual close never spills a later single-ring skim into documents.
    this.dispatch("closed")
  }

  // After skimming, refresh the inbox tray once so addressed/archived stacks drop
  // and new pins appear (Keep doesn't broadcast per-card, to avoid a storm).
  refreshTray() {
    const tray = document.getElementById("skim_tray")
    if (tray && typeof tray.reload === "function") tray.reload()
  }

  backdropClose(event) {
    if (event.target === event.currentTarget) this.close()
  }

  onKeydown(event) {
    if (event.key === "Escape") this.close()
  }

  // Fired by the frame's turbo:frame-load — focus the viewer so arrow keys work
  // immediately (story-style, no click needed).
  frameLoaded() {
    this.focusStack()
  }

  focusStack() {
    const stack = this.panelTarget.querySelector('[data-controller~="skim-mode"]')
    if (stack) stack.focus()
  }
}
