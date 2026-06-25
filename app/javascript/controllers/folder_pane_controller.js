import { Controller } from "@hotwired/stimulus"

// Toggles the inbox folder pane between its expanded panel and a slim icon rail,
// remembering the choice across loads (mirrors email_sidebar, plus persistence).
// The panel renders expanded by default; on connect we restore a stored collapse.
export default class extends Controller {
  static targets = ["panel", "rail"]
  static values = { storageKey: { type: String, default: "campbooks:folder-pane-collapsed" } }

  connect() {
    if (this.#stored() === "1") this.#apply(true)
  }

  collapse() { this.#apply(true); this.#persist(true) }
  expand() { this.#apply(false); this.#persist(false) }

  #apply(collapsed) {
    if (!this.hasPanelTarget || !this.hasRailTarget) return
    this.panelTarget.classList.toggle("hidden", collapsed)
    this.panelTarget.classList.toggle("flex", !collapsed)
    this.railTarget.classList.toggle("hidden", !collapsed)
    this.railTarget.classList.toggle("flex", collapsed)
  }

  #persist(collapsed) {
    try { localStorage.setItem(this.storageKeyValue, collapsed ? "1" : "0") } catch (_) { /* private mode */ }
  }

  #stored() {
    try { return localStorage.getItem(this.storageKeyValue) } catch (_) { return null }
  }
}
