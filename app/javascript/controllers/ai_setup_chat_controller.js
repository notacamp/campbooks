import { Controller } from "@hotwired/stimulus"

// Minimal chat behaviour for the conversational setup dialog: Enter-to-send,
// autoscroll as new turns stream in, and clearing the box after each send.
// (The shared `chat-input` controller assumes one wrapping form; here the
// proposal renders its own form, so the message form and proposal form must be
// siblings — hence a small dedicated controller.)
export default class extends Controller {
  static targets = ["list", "input"]

  connect() {
    this.scrollToBottom()
    this._onRender = () => setTimeout(() => this.scrollToBottom(), 50)
    document.addEventListener("turbo:before-stream-render", this._onRender)
  }

  disconnect() {
    document.removeEventListener("turbo:before-stream-render", this._onRender)
  }

  keydown(event) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      this.inputTarget.closest("form")?.requestSubmit()
    }
  }

  reset() {
    if (this.hasInputTarget) this.inputTarget.value = ""
  }

  scrollToBottom() {
    if (this.hasListTarget) this.listTarget.scrollTop = this.listTarget.scrollHeight
  }
}
