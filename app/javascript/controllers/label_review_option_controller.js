import { Controller } from "@hotwired/stimulus"

// Enables/disables the tag-select dropdown in a label review row based on
// whether the "Map to existing tag" radio button is chosen.
export default class extends Controller {
  static targets = ["radio", "select"]

  connect() {
    this.toggle()
  }

  toggle() {
    const enabled = this.radioTarget.checked
    this.selectTarget.disabled = !enabled
    if (!enabled) this.selectTarget.value = ""
  }
}
