import { Controller } from "@hotwired/stimulus"

// Closes the nearest <dialog> the moment this element is inserted into the DOM.
// Rendered into a turbo-frame by a Turbo Stream so a server action can dismiss
// a modal after it succeeds (e.g. the setup wizards).
export default class extends Controller {
  connect() {
    this.element.closest("dialog")?.close()
  }
}
