import { Controller } from "@hotwired/stimulus"

// Submits the controller's form whenever a wired input changes — e.g. a role
// <select> in the email-account sharing panel — so there's no extra "Save" click.
// CSP-safe: uses a data-action binding instead of an inline onchange handler.
export default class extends Controller {
  submit() {
    this.element.requestSubmit()
  }
}
