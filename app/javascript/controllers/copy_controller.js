import { Controller } from "@hotwired/stimulus"

// Copies arbitrary text to the clipboard. Text is passed via a Stimulus param:
//   data-action="click->copy#copy"
//   data-copy-text-param="some@email.com"
export default class extends Controller {
  copy ({ params: { text } }) {
    if (!text) return
    navigator.clipboard.writeText(text).catch(() => {})
  }
}
