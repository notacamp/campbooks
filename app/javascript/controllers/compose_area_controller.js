import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  connect() {
    this.csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    this.boundRestoreButton = this.restoreButton.bind(this)
    this.element.addEventListener("turbo:submit-end", this.boundRestoreButton)
    this._revealComposer()
  }

  // When the composer is injected (e.g. clicking Reply in the email drawer),
  // scroll it into view within its scroll container and focus the right field:
  // the recipient when it's empty (forward / new message), otherwise the message
  // body with the caret placed above the quoted text. Deferred a frame so the
  // TipTap editor has mounted its contenteditable.
  _revealComposer() {
    requestAnimationFrame(() => {
      const to = this.element.querySelector('input[name="to_address"]')
      if (to && !to.value.trim()) {
        const recipient = this.element.querySelector("[data-contact-pill-input-target='search']") || to
        recipient?.focus()
        recipient?.scrollIntoView({ behavior: "smooth", block: "nearest" })
        return
      }

      const editable = this.element.querySelector("[data-tiptap-editor-target='editor'] [contenteditable='true'], .ProseMirror")
      if (editable) {
        editable.focus()
        this._caretToStart(editable)
      } else {
        this.element.scrollIntoView({ behavior: "smooth", block: "nearest" })
      }
    })
  }

  _caretToStart(editable) {
    const selection = window.getSelection()
    if (!selection) return
    const range = document.createRange()
    range.setStart(editable, 0)
    range.collapse(true)
    selection.removeAllRanges()
    selection.addRange(range)
  }

  disconnect() {
    if (this.boundRestoreButton) {
      this.element.removeEventListener("turbo:submit-end", this.boundRestoreButton)
    }
  }

  validateForm(event) {
    const toInput = this.element.querySelector('input[name="to_address"]')
    if (!toInput || !toInput.value.trim()) {
      event.preventDefault()
      // Find the visible search input for focus
      const searchInput = this.element.querySelector('[data-contact-pill-input-target="search"]')
      if (searchInput) {
        searchInput.closest("[data-controller~='contact-pill-input']")?.querySelector(".border-gray-200")?.classList.add("border-red-400", "ring-1", "ring-red-400")
        searchInput.focus()
      }
      return false
    }
    // Show sending state
    this.element.querySelector("button[type=submit]")?.setAttribute("disabled", "disabled")
    this.showSending()
  }

  restoreButton() {
    const btn = this.element.querySelector("button[type=submit]")
    if (btn) {
      btn.removeAttribute("disabled")
      btn.classList.remove("opacity-60")
      btn.innerHTML = `<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/></svg> Send`
    }
  }

  showSending() {
    const btn = this.element.querySelector("button[type=submit]")
    if (btn) {
      btn.innerHTML = `<svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
      </svg> Sending…`
      btn.classList.add("opacity-60")
    }
  }

  selectSignature(event) {
    const select = event.target
    const selectedOption = select.options[select.selectedIndex]
    const signatureHtml = selectedOption?.dataset?.content || ""
    const signatureId = select.value

    this._updateSignaturePreview(signatureId, signatureHtml)
  }

  _updateSignaturePreview(signatureId, signatureHtml) {
    let preview = this.element.querySelector(".email-signature-preview")
    if (!preview) return

    if (!signatureId || !signatureHtml) {
      preview.innerHTML = ""
      return
    }

    preview.innerHTML = `<div class="text-[10px] text-gray-400 mb-1 uppercase tracking-wide">Signature</div><div class="text-xs text-gray-500 border border-gray-200 rounded-md p-2.5 bg-gray-50 max-h-20 overflow-y-auto">${signatureHtml}</div>`
  }

  discard(event) {
    event.preventDefault()
    // The compose area is purely client-side UI state, so just remove its wrapper.
    // (This previously POSTed and relied on a raw fetch to apply a Turbo Stream —
    // which Turbo never processes from fetch — so the area was never removed.)
    ;(this.element.closest("[id^='compose_area_']") || this.element).remove()
  }
}
