import { Controller } from "@hotwired/stimulus"

// A close/back link that returns the user to wherever they came from, instead of
// a fixed destination. Used by full-page (standalone) views — e.g. Skim opened
// from the home feed — where a hardcoded href would always dump the user at a
// section's home rather than the page they came from.
//
// If there's in-app history to return to, go back: a Turbo restoration visit, so
// the previous page AND its scroll position are restored (e.g. their place in the
// infinite-scroll feed). Otherwise fall through to the element's own href — the
// section home — which covers a direct visit or a freshly opened tab.
export default class extends Controller {
  back(event) {
    if (window.history.length > 1) {
      event.preventDefault()
      window.history.back()
    }
  }
}
