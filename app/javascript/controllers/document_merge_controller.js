import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["bar", "count", "mergeLink"]

  connect() {
    this.selected = new Set()
  }

  toggle(event) {
    const cb = event.target
    if (cb.checked) {
      this.selected.add(cb.value)
    } else {
      this.selected.delete(cb.value)
    }
    this.updateUI()
  }

  toggleAll(event) {
    const checked = event.target.checked
    const checkboxes = this.element.querySelectorAll(".document-merge-checkbox")
    checkboxes.forEach(cb => {
      cb.checked = checked
      if (checked) {
        this.selected.add(cb.value)
      } else {
        this.selected.delete(cb.value)
      }
    })
    this.updateUI()
  }

  updateUI() {
    if (this.selected.size >= 2) {
      this.barTarget.classList.remove("hidden")
      this.countTarget.textContent = `${this.selected.size}`
      this.mergeLinkTarget.href = `/documents/merge?ids=${[...this.selected].join(",")}`
    } else {
      this.barTarget.classList.add("hidden")
    }
  }
}
