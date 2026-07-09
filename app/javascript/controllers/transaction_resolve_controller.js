import { Controller } from "@hotwired/stimulus"

// Handles the "Resolve" button for unmatched bank transactions.
//
// Desktop: inserts a sibling <tr> after the transaction row and lazy-loads a
// Turbo Frame into it. Clicking "Resolve" again collapses the row.
//
// Mobile/card: appends an inline section to the card div and lazy-loads the
// same Turbo Frame into it.
//
// The outer Turbo Frame ID is "<element.id>_resolve_frame" which matches the
// <turbo-frame> tag in resolve_panel.html.erb.
// The controller element can be a <tr> (desktop) or a <div> (mobile card).
export default class extends Controller {
  static values = { url: String }

  toggle(event) {
    event.preventDefault()

    const panelId  = `${this.element.id}_resolve_panel`
    const frameId  = `${this.element.id}_resolve_frame`

    let panel = document.getElementById(panelId)

    if (panel) {
      // Toggle visibility
      const isHidden = panel.classList.toggle("hidden")
      if (!isHidden) this._loadFrame(panel, frameId)
      return
    }

    // First open: create the panel
    panel = this._createPanel(panelId, frameId)
    this._loadFrame(panel, frameId)
  }

  // Close the panel from outside (e.g. after a confirm/reject action replaces the row).
  close() {
    const panel = document.getElementById(`${this.element.id}_resolve_panel`)
    if (panel) panel.classList.add("hidden")
  }

  // ── Private ──────────────────────────────────────────────────────────────────

  _createPanel(panelId, frameId) {
    if (this.element.tagName === "TR") {
      // Desktop: sibling <tr> after the main transaction row
      const tr = document.createElement("tr")
      tr.id = panelId

      const td = document.createElement("td")
      td.colSpan = 99
      td.className = "p-0"

      const frame = this._createFrame(frameId)
      frame.className = "block"
      td.appendChild(frame)
      tr.appendChild(td)
      this.element.after(tr)
      return tr
    } else {
      // Mobile card: appended section inside the card div
      const div = document.createElement("div")
      div.id = panelId
      div.className = "border-t border-border"

      const frame = this._createFrame(frameId)
      frame.className = "block"
      div.appendChild(frame)
      this.element.appendChild(div)
      return div
    }
  }

  _createFrame(frameId) {
    const frame = document.createElement("turbo-frame")
    frame.id = frameId
    return frame
  }

  _loadFrame(panel, frameId) {
    const frame = panel.querySelector("turbo-frame")
    if (frame && !frame.getAttribute("src")) {
      frame.setAttribute("src", this.urlValue)
    }
  }
}
