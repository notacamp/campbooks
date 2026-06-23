import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = [ "input", "submit", "list" ]
  connect() {
    this.scrollToBottom()

    this.observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.addedNodes.forEach((node) => {
          if (node.nodeType === 1) {
            const messages = node.querySelectorAll?.(".chat-message") || []
            messages.forEach((msg) => {
              msg.classList.add("new-message-glow")
              setTimeout(() => msg.classList.remove("new-message-glow"), 2000)
            })
            if (node.classList?.contains("chat-message")) {
              node.classList.add("new-message-glow")
              setTimeout(() => node.classList.remove("new-message-glow"), 2000)
            }
          }
        })
      })
      this.scrollToBottom()
    })
    if (this.hasListTarget) {
      this.observer.observe(this.listTarget, { childList: true, subtree: true })
    }

    this.syncSubmit()
    document.addEventListener("turbo:before-stream-render", this._onStreamRender)
  }

  disconnect() {
    this.observer?.disconnect()
    document.removeEventListener("turbo:before-stream-render", this._onStreamRender)
  }

  _onStreamRender = (event) => {
    setTimeout(() => this.scrollToBottom(), 50)
  }

  keydown(event) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      this.submitForm()
    }
  }

  submit(event) {
    event.preventDefault()
    this.submitForm()
  }

  // Tappable suggestion chips / briefing cards drop their text into the
  // composer and send it. Works from any chat-input scope on the page (the
  // chips live in the message-list scope, the textarea in the form scope), so
  // we locate the composer by its target rather than relying on `this`.
  prompt(event) {
    event.preventDefault()
    const text = event.params?.text || event.currentTarget?.dataset?.chatInputTextParam
    if (!text) return

    const input = document.querySelector('[data-chat-input-target="input"]')
    const form = input?.closest("form")
    if (!input || !form) return

    this.clearFollowups()
    input.value = text
    this.autosize(input)
    form.requestSubmit()
  }

  submitForm() {
    const input = this.inputTarget
    if (!input.value.trim()) return

    this.clearFollowups()

    if (this.hasSubmitTarget) {
      this.submitTarget.disabled = true
      // Keep an icon button as-is; only swap text on a plain text button.
      if (!this.submitTarget.querySelector("svg")) this.submitTarget.textContent = "Sending…"
    }

    this.element.requestSubmit()
  }

  // Auto-grow the textarea up to its max height as the user types, and keep the
  // send button's enabled state in sync with whether there's anything to send.
  grow(event) {
    this.autosize(event?.target || this.inputTarget)
    this.syncSubmit()
  }

  autosize(el) {
    if (!el) return
    el.style.height = "auto"
    el.style.height = Math.min(el.scrollHeight, 160) + "px"
  }

  // The send button is disabled while the composer is empty, so it reads as
  // "nothing to send yet" rather than an always-lit control that does nothing.
  syncSubmit() {
    if (this.hasSubmitTarget && this.hasInputTarget) {
      this.submitTarget.disabled = !this.inputTarget.value.trim()
    }
  }

  // Stale follow-up chips (from a previous reply) should vanish the moment the
  // user moves on, so only the latest reply ever shows next steps.
  clearFollowups() {
    document.querySelectorAll("[data-followups]").forEach((el) => el.remove())
  }

  scrollToBottom() {
    if (this.hasListTarget) {
      this.listTarget.scrollTop = this.listTarget.scrollHeight
    }
  }
}
