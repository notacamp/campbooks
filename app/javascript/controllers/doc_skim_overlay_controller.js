import { Controller } from "@hotwired/stimulus"

// Opens document Skim as a full-screen, stories-style overlay. The viewer loads
// into a turbo-frame the first time and re-fetches on every subsequent open so it
// reflects the current review queue (e.g. after approving). "Review all" opens at
// the first ring; a tray ring deep-links to its category. Escape, the in-viewer
// close button, or the backdrop dismiss it. The document-world analogue of
// skim_overlay_controller — scoped to the /documents page, not mounted globally.
export default class extends Controller {
  static targets = ["panel", "frame"]

  open(event) {
    if (event) event.preventDefault()
    this.reveal()
    this.loadFrame(this.base)
  }

  // Deep-link open from a tray ring: start the viewer on a given category.
  openTo(event) {
    if (event) event.preventDefault()
    this.reveal()
    const category = event && event.params ? event.params.category : null
    this.loadFrame(category && this.base ? `${this.base}?start=${encodeURIComponent(category)}` : this.base)
  }

  get base() { return this.hasFrameTarget ? this.frameTarget.dataset.docSkimSrc : null }

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
  }

  // After reviewing, refresh the /documents tray once so cleared categories drop
  // and counts update (the controller also broadcasts live, but this covers the
  // local session immediately on close).
  refreshTray() {
    const tray = document.getElementById("doc_skim_tray")
    if (tray && typeof tray.reload === "function") tray.reload()
  }

  backdropClose(event) {
    if (event.target === event.currentTarget) this.close()
  }

  onKeydown(event) {
    if (event.key === "Escape") this.close()
  }

  // Fired by the frame's turbo:frame-load — focus the viewer so the keyboard
  // shortcuts work immediately (no click needed).
  frameLoaded() {
    this.focusStack()
  }

  focusStack() {
    const stack = this.panelTarget.querySelector('[data-controller~="doc-skim-mode"]')
    if (stack) stack.focus()
  }
}
