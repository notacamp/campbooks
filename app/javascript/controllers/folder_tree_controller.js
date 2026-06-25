import { Controller } from "@hotwired/stimulus"

// Collapses / expands a folder's children in the pane tree. Each expandable
// folder + its children container is wrapped in its own folder-tree controller,
// so the chevron toggles just that subtree. Subtrees start expanded.
export default class extends Controller {
  static targets = ["children"]

  toggle(e) {
    const hidden = this.childrenTarget.classList.toggle("hidden")
    const chevron = e.currentTarget.querySelector("svg")
    if (chevron) chevron.classList.toggle("rotate-90", !hidden)
    e.currentTarget.setAttribute("aria-expanded", String(!hidden))
  }
}
