import { Controller } from "@hotwired/stimulus"

// Toggles the AI setup step between "Campbooks AI" (managed) and "bring your own
// keys" panels based on the chosen radio. The radios carry name="ai_mode", so the
// selected value submits with the form directly — this controller only reveals the
// matching panel.
export default class extends Controller {
  static targets = ["managedPanel", "byoPanel"]

  connect() {
    const checked = this.element.querySelector('input[name="ai_mode"]:checked')
    if (checked) this.applyMode(checked.value)
  }

  select(event) {
    this.applyMode(event.target.value)
  }

  applyMode(mode) {
    if (this.hasManagedPanelTarget) {
      this.managedPanelTarget.classList.toggle("hidden", mode !== "managed")
    }
    if (this.hasByoPanelTarget) {
      this.byoPanelTarget.classList.toggle("hidden", mode !== "byo")
    }
  }
}
