import { Controller } from "@hotwired/stimulus"

/**
 * Handles the multi-select persona card picker in the onboarding template step.
 *
 * Rules:
 *  - Any combination of non-exclusive cards can be checked simultaneously.
 *  - The "Just exploring" card (data-exclusive="true") is mutually exclusive
 *    with all others: checking it unchecks everything else, and checking any
 *    other card automatically unchecks it.
 *  - The continue button stays enabled at all times (the user can proceed with
 *    no selection via the Skip link, or with any number of selections).
 */
export default class extends Controller {
  static targets = ["card", "continueBtn"]

  toggle(event) {
    const changed = event.currentTarget
    const isExclusive = changed.dataset.exclusive === "true"

    if (isExclusive && changed.checked) {
      // Exclusive card just turned on → uncheck all others
      this.#allCheckboxes().forEach(cb => {
        if (cb !== changed) cb.checked = false
      })
    } else if (!isExclusive && changed.checked) {
      // A normal card just turned on → uncheck the exclusive one
      this.#allCheckboxes().forEach(cb => {
        if (cb.dataset.exclusive === "true") cb.checked = false
      })
    }
  }

  // ── Private ──────────────────────────────────────────────

  #allCheckboxes() {
    return Array.from(this.element.querySelectorAll("input[type=checkbox]"))
  }
}
