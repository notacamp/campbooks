import { Controller } from "@hotwired/stimulus"

// Scrolls the element into view as soon as it connects — used by the People
// conversation to jump to a focused thread when arriving via a message permalink.
export default class extends Controller {
  connect() {
    this.element.scrollIntoView({ block: "start" })
  }
}
