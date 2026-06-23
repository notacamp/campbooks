import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["panel", "list"]

  connect() {
    this.boundClickOutside = this.clickOutside.bind(this)
    document.addEventListener("click", this.boundClickOutside)
  }

  disconnect() {
    document.removeEventListener("click", this.boundClickOutside)
  }

  toggle(event) {
    event.stopPropagation()
    this.panelTarget.classList.toggle("hidden")
    if (!this.panelTarget.classList.contains("hidden")) {
      this.fetchLatest()
    }
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this.panelTarget.classList.add("hidden")
    }
  }

  fetchLatest() {
    fetch("/notifications?format=turbo_stream&per_page=10")
      .then(response => response.text())
      .then(html => {
        Turbo.renderStreamMessage(html)
      })
  }

  close() {
    this.panelTarget.classList.add("hidden")
  }
}
