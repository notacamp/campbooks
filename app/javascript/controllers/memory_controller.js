import { Controller } from "@hotwired/stimulus"

// Client-side search filter for Scout's memory. Plain-text substring match over
// each row's data-memory-text; no server round-trip (the facet chips handle
// server-side filtering). Toggles inline display so it beats the row's own
// flex/grid display classes.
export default class extends Controller {
  static targets = ["search", "row", "empty"]

  filter() {
    const query = this.searchTarget.value.trim().toLowerCase()
    let visible = 0

    this.rowTargets.forEach((row) => {
      const match = query === "" || (row.dataset.memoryText || "").includes(query)
      row.style.display = match ? "" : "none"
      if (match) visible++
    })

    if (this.hasEmptyTarget) {
      this.emptyTarget.classList.toggle("hidden", !(query !== "" && visible === 0))
    }
  }
}
