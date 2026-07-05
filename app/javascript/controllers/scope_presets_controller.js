import { Controller } from "@hotwired/stimulus"

// One-click scope selections for the API-client scope picker. Each preset
// button carries the space-separated scope list it stands for; applying it
// checks exactly those boxes (so "Clear" is just an empty list).
export default class extends Controller {
  static targets = ["checkbox"]

  apply(event) {
    const list = (event.params.list || "").split(/\s+/).filter(Boolean)
    this.checkboxTargets.forEach((box) => {
      box.checked = list.includes(box.value)
    })
  }
}
