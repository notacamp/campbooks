import { Controller } from "@hotwired/stimulus"

// The bold composer's intent input (Campbooks::Compose::IntentInput).
//
// Enter or the Draft button posts the note to the hidden compose-chat thread,
// whose auto-actions fill the editor and envelope. While Scout works the button
// shows a spinner; it restores when the body arrives (compose-chat:body-set).
//
// The input lives *inside* the compose <form>, so its own typing is kept local
// (stopPropagation) — intent notes must not trip draft autosave. A 2+ sentence
// note left untouched becomes the message itself when focus moves into the body.
export default class extends Controller {
  static targets = ["input", "draftButton", "spinner", "label"]

  connect() {
    this._working = false
  }

  // Intent typing is not an email edit — keep it from reaching autosave/engine.
  localInput(event) {
    event.stopPropagation()
  }

  keydown(event) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      event.stopPropagation()
      this.draft()
    }
  }

  draft() {
    if (this._working) return
    const text = this.inputTarget.value.trim()
    if (!text) return

    const chat = this._composeChat()
    if (!chat || typeof chat.submitIntent !== "function") return
    if (chat.submitIntent(text) === false) return

    this._working = true
    if (this.hasSpinnerTarget) this.spinnerTarget.classList.remove("hidden")
    if (this.hasLabelTarget) this.labelTarget.classList.add("hidden")
    if (this.hasDraftButtonTarget) this.draftButtonTarget.setAttribute("disabled", "disabled")
  }

  // Scout's draft landed in the editor → let the writer edit again.
  restore() {
    this._working = false
    if (this.hasSpinnerTarget) this.spinnerTarget.classList.add("hidden")
    if (this.hasLabelTarget) this.labelTarget.classList.remove("hidden")
    if (this.hasDraftButtonTarget) this.draftButtonTarget.removeAttribute("disabled")
  }

  // A long note (2+ sentences) left as-is becomes the message when the writer
  // clicks into the still-empty body — "both produce the draft below".
  maybeMoveToBody(event) {
    const to = event.relatedTarget
    if (!to) return
    if (this.hasDraftButtonTarget && (to === this.draftButtonTarget || this.draftButtonTarget.contains(to))) return

    const intoEditor = to.closest?.("[data-controller~='tiptap-editor'], .ProseMirror")
    if (!intoEditor) return

    const text = this.inputTarget.value.trim()
    if (this._sentenceCount(text) < 2) return

    const editor = this._editor()
    if (!editor) return
    // Never clobber a body the writer (or Scout) already has.
    if (new DOMParser().parseFromString(editor.getHTML?.() || "", "text/html").body.textContent.trim()) return

    const html = text
      .split(/\n{2,}/)
      .map((p) => `<p>${this._escape(p).replace(/\n/g, "<br>")}</p>`)
      .join("")
    editor.setContent(html) // _sync() fires a bubbling input → autosave sees real content
    this.inputTarget.value = ""
  }

  // ── internals ──────────────────────────────────────────────
  _composeChat() {
    const el = document.querySelector("[data-controller~='compose-chat']")
    return el && this.application.getControllerForElementAndIdentifier(el, "compose-chat")
  }

  _editor() {
    const el = document.querySelector("[data-controller~='tiptap-editor']")
    return el && this.application.getControllerForElementAndIdentifier(el, "tiptap-editor")
  }

  _sentenceCount(text) {
    const terminators = text.match(/[.!?]+(\s|$)/g)
    if (terminators && terminators.length >= 1) return terminators.length
    // No terminal punctuation: a line break also reads as a break between thoughts.
    return text.includes("\n") ? 2 : 1
  }

  _escape(str) {
    const el = document.createElement("div")
    el.textContent = str
    return el.innerHTML
  }
}
