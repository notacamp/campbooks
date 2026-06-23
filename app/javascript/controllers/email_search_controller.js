import { Controller } from "@hotwired/stimulus"

// Drives the inbox search bar. The <form> itself navigates the
// `email_search_results` Turbo Frame (data-turbo-frame), so there is no manual
// fetch here — this controller only decides *when* to submit and toggles the
// filter panel.
//
// - text input: debounced submit on `input`, immediate on Enter
// - any filter change (select / checkbox / toggle / date): immediate submit,
//   wired at the form level via `change->email-search#submitNow`
// - Filters button: toggles the panel
// - tag filter box: client-side show/hide of the tag checkboxes
export default class extends Controller {
  static targets = ["query", "filterPanel", "tagOption"]
  static values = { debounce: { type: Number, default: 300 } }

  scheduleSubmit() {
    clearTimeout(this.timer)
    this.timer = setTimeout(() => this.element.requestSubmit(), this.debounceValue)
  }

  handleKeydown(event) {
    if (event.key === "Enter") {
      event.preventDefault()
      this.submitNow()
    }
  }

  submitNow() {
    clearTimeout(this.timer)
    this.element.requestSubmit()
  }

  toggleFilters() {
    this.filterPanelTarget.classList.toggle("hidden")
  }

  // Client-side filter of the tag checkbox list — no request.
  filterTags(event) {
    const term = event.target.value.trim().toLowerCase()
    this.tagOptionTargets.forEach((el) => {
      const name = el.dataset.tagName || ""
      el.classList.toggle("hidden", term !== "" && !name.includes(term))
    })
  }

  disconnect() {
    clearTimeout(this.timer)
  }
}
