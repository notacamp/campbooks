import { Controller } from "@hotwired/stimulus"

// In-place document preview for the reconciliation workbench.
//
// Intercepts PLAIN left-clicks on any /documents/:id link inside the page and
// opens the preview drawer with the document's file iframe instead of
// navigating. Modified clicks (ctrl/cmd/shift/middle) and links marked
// data-document-preview-skip fall through to normal navigation, so "open in
// new tab" always remains available.
//
// The iframe loads /documents/:id/file (the blob proxy used by the document
// show page), and the header's "open full page" link points at the original
// href.
export default class extends Controller {
  static targets = ["backdrop", "panel", "iframe", "title", "fullPageLink"]

  connect() {
    this.boundClick = this._interceptClick.bind(this)
    this.boundKeydown = this._keydown.bind(this)
    // Capture phase: beat Turbo's own click handling, same trick as email-drawer.
    document.addEventListener("click", this.boundClick, true)
    document.addEventListener("keydown", this.boundKeydown)
  }

  disconnect() {
    document.removeEventListener("click", this.boundClick, true)
    document.removeEventListener("keydown", this.boundKeydown)
  }

  close() {
    this.panelTarget.classList.add("translate-y-full", "sm:translate-x-full")
    this.panelTarget.classList.remove("translate-y-0", "sm:translate-x-0")
    this.backdropTarget.style.display = "none"
    document.body.classList.remove("overflow-hidden")
    this.hideTimeout = setTimeout(() => {
      this.panelTarget.classList.add("invisible")
      this.iframeTarget.src = "about:blank" // drop the loaded PDF
    }, 300)
  }

  _interceptClick(event) {
    if (event.defaultPrevented) return
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

    const link = event.target.closest("a[href]")
    if (!link) return
    if (link.dataset.documentPreviewSkip) return
    if (!this.element.contains(link) && !link.closest("[data-document-previewable]")) {
      // Only intercept links inside regions that opted in.
      return
    }

    const match = link.getAttribute("href").match(/^\/documents\/([0-9a-f-]{36})$/)
    if (!match) return

    event.preventDefault()
    event.stopPropagation()
    this._open(match[1], link)
  }

  _open(documentId, link) {
    clearTimeout(this.hideTimeout)
    this.titleTarget.textContent = link.dataset.documentPreviewTitle ||
      link.getAttribute("title") || link.textContent.trim()
    this.fullPageLinkTarget.setAttribute("href", link.getAttribute("href"))
    this.iframeTarget.src = `/documents/${documentId}/file`

    this.panelTarget.classList.remove("invisible", "translate-y-full", "sm:translate-x-full")
    this.panelTarget.classList.add("translate-y-0", "sm:translate-x-0")
    this.backdropTarget.style.display = "block"
    document.body.classList.add("overflow-hidden")
  }

  _keydown(event) {
    if (event.key === "Escape" && !this.panelTarget.classList.contains("invisible")) this.close()
  }
}
