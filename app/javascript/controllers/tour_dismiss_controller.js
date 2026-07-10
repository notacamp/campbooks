import { Controller } from "@hotwired/stimulus"

// Dismisses a one-time banner that is guarded server-side by User#tour_dismissed?
// (the `accounting_nif_banner`, setup coachmarks, etc.).
//
// Usage:
//   <div data-controller="tour-dismiss" data-tour-dismiss-url-value="<%= dismiss_tour_path('key') %>">
//     ...
//     <button type="button" data-action="tour-dismiss#dismiss">Dismiss</button>
//   </div>
//
// On dismiss:
//   1. The element is removed from the DOM immediately (no visible delay).
//   2. A background POST to `urlValue` records the dismissal server-side so
//      the banner is never rendered again (User#tour_dismissed?).
export default class extends Controller {
  static values = { url: String }

  dismiss() {
    this.element.remove()

    if (this.urlValue) {
      fetch(this.urlValue, {
        method: "POST",
        headers: {
          "X-CSRF-Token": document.querySelector('meta[name="csrf-token"]')?.content || "",
          "Accept": "application/json"
        }
      }).catch(() => {})
    }
  }
}
