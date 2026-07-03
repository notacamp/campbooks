import { Controller } from "@hotwired/stimulus"

// Shell-agnostic behavior of the composer engine (Campbooks::Compose::Engine):
// envelope collapse/expand, Cc/Bcc reveal, quoted-thread expansion, submit
// validation + busy state, ⌘↵ send, focus-on-open, and discard. Draft
// persistence lives in the sibling compose-autosave controller on the same
// <form>; shells listen for the bubbled `compose-engine:closed` event.
export default class extends Controller {
  static targets = [
    "summary", "summaryRecipients", "summarySubject", "fields",
    "ccRow", "bccRow", "ccToggle", "bccToggle",
    "subjectInput", "collapseButton", "quoteWrap", "quotedInput", "sendButton",
    "scoutDraft", "scoutText", "scoutSpark"
  ]
  static values = { messageId: { type: String, default: "" } }

  connect() {
    this._focusInitial()
    this._syncCollapseButton()
  }

  // ── envelope ─────────────────────────────────────────────────
  expandEnvelope() {
    if (this.hasSummaryTarget) this.summaryTarget.classList.add("hidden")
    if (this.hasFieldsTarget) this.fieldsTarget.classList.remove("hidden")
    this._syncCollapseButton()
    this._focusSearch("to_address")
  }

  collapseEnvelope() {
    if (!this._complete()) return
    this._refreshSummary()
    if (this.hasFieldsTarget) this.fieldsTarget.classList.add("hidden")
    if (this.hasSummaryTarget) this.summaryTarget.classList.remove("hidden")
  }

  showCc(event) {
    event?.preventDefault()
    this.ccRowTarget.classList.remove("hidden")
    if (this.hasCcToggleTarget) this.ccToggleTarget.classList.add("hidden")
    this._focusSearch("cc_address")
  }

  showBcc(event) {
    event?.preventDefault()
    this.bccRowTarget.classList.remove("hidden")
    if (this.hasBccToggleTarget) this.bccToggleTarget.classList.add("hidden")
    this._focusSearch("bcc_address")
  }

  // ── quoted thread ────────────────────────────────────────────
  // Folds the quoted original into the editor for editing and drops the pill.
  expandQuote(event) {
    event?.preventDefault()
    if (!this.hasQuotedInputTarget) return
    const html = this.quotedInputTarget.value
    if (html) this._editorController()?.appendContent(html)
    this.quotedInputTarget.value = ""
    if (this.hasQuoteWrapTarget) this.quoteWrapTarget.remove()
    this.element.dispatchEvent(new Event("input", { bubbles: true }))
  }

  // ── submit ───────────────────────────────────────────────────
  validate(event) {
    const to = this.element.querySelector('input[name="to_address"]')
    if (!to || !to.value.trim()) {
      event.preventDefault()
      this.expandEnvelope()
      const pills = this.element.querySelector("[data-contact-pill-input-target='pills']")
      pills?.classList.add("ring-1", "ring-red-400", "rounded-md")
      setTimeout(() => pills?.classList.remove("ring-1", "ring-red-400", "rounded-md"), 2500)
      return false
    }
    this._showSubmitting(event.submitter)
  }

  restoreButton() {
    const btn = this._pendingButton || (this.hasSendButtonTarget ? this.sendButtonTarget : null)
    if (!btn) return
    btn.removeAttribute("disabled")
    btn.classList.remove("opacity-60")
    if (btn.dataset.originalHtml) {
      btn.innerHTML = btn.dataset.originalHtml
      delete btn.dataset.originalHtml
    }
    this._pendingButton = null
  }

  keydown(event) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault()
      this.element.requestSubmit(this.hasSendButtonTarget ? this.sendButtonTarget : undefined)
    }
  }

  // ── Scout ghost draft (Probe 02) ─────────────────────────────
  // Take ownership: the ghost's text becomes editor content (plain ink).
  useScoutDraft(event) {
    event?.preventDefault()
    if (!this.hasScoutTextTarget) return
    const text = this.scoutTextTarget.textContent.trim()
    const html = text.split(/\n{2,}/).map((p) =>
      `<p>${this._escapeHtml(p).replace(/\n/g, "<br>")}</p>`
    ).join("")
    this._editorController()?.setContent(html)
    this.dismissScoutDraft()
    this.element.dispatchEvent(new Event("input", { bubbles: true }))
    this.element.querySelector(".ProseMirror")?.focus()
  }

  dismissScoutDraft(event) {
    event?.preventDefault()
    if (this.hasScoutDraftTarget) this.scoutDraftTarget.remove()
  }

  // Ask Scout to draft this reply (footer spark), or regenerate with a tone
  // instruction (ghost chips). The stream replaces #compose_scout_slot.
  requestScoutDraft(event) {
    event?.preventDefault()
    this._fetchScoutDraft()
  }

  retoneScoutDraft(event) {
    event?.preventDefault()
    const tone = event.params.tone
    const current = this.hasScoutTextTarget ? this.scoutTextTarget.textContent.trim() : ""
    this._fetchScoutDraft(`Rewrite this draft to be ${tone}, keeping the same facts:\n${current}`)
  }

  _fetchScoutDraft(summary = "") {
    if (!this.messageIdValue) return
    if (this.hasScoutSparkTarget) this.scoutSparkTarget.classList.add("animate-pulse")
    if (this.hasScoutDraftTarget) this.scoutDraftTarget.classList.add("opacity-50", "pointer-events-none")

    const params = new URLSearchParams({ tool: "draft_reply", surface: "dock" })
    if (summary) params.set("args[summary]", summary)
    fetch(`/email_messages/${this.messageIdValue}/tool`, {
      method: "POST",
      headers: {
        "X-CSRF-Token": document.querySelector("meta[name='csrf-token']")?.content || "",
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: params.toString()
    }).then((r) => r.text()).then((html) => {
      if (html) Turbo.renderStreamMessage(html)
    }).finally(() => {
      if (this.hasScoutSparkTarget) this.scoutSparkTarget.classList.remove("animate-pulse")
    })
  }

  _escapeHtml(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }

  // ── discard ──────────────────────────────────────────────────
  discard(event) {
    event?.preventDefault()
    this._autosaveController()?.discard()
    this.element.dispatchEvent(new CustomEvent("compose-engine:closed", { bubbles: true }))
  }

  // ── internals ────────────────────────────────────────────────
  _complete() {
    const to = this.element.querySelector('input[name="to_address"]')?.value?.trim()
    const subject = this.hasSubjectInputTarget ? this.subjectInputTarget.value.trim() : ""
    return Boolean(to && subject)
  }

  _refreshSummary() {
    if (this.hasSummaryRecipientsTarget) {
      const to = this.element.querySelector('input[name="to_address"]')?.value || ""
      const list = to.split(",").map((s) => s.trim()).filter(Boolean)
      const first = (list[0] || "").replace(/<.*$/, "").trim() || list[0] || ""
      const extra = list.length - 1
      this.summaryRecipientsTarget.textContent = extra > 0 ? `${first} +${extra}` : first
    }
    if (this.hasSummarySubjectTarget && this.hasSubjectInputTarget) {
      this.summarySubjectTarget.textContent = `· ${this.subjectInputTarget.value.trim()}`
    }
  }

  // The dock's subject row shows a collapse chevron once the envelope is
  // complete; listens on the form's input event stream.
  _syncCollapseButton() {
    if (!this.hasCollapseButtonTarget) return
    this.collapseButtonTarget.classList.toggle("hidden", !this._complete())
  }

  changedAnywhere() {
    this._syncCollapseButton()
  }

  _focusInitial() {
    requestAnimationFrame(() => {
      const to = this.element.querySelector('input[name="to_address"]')
      const fieldsHidden = this.hasFieldsTarget && this.fieldsTarget.classList.contains("hidden")
      if (to && !to.value.trim() && !fieldsHidden) {
        this._focusSearch("to_address")
        return
      }
      const editable = this.element.querySelector(".ProseMirror, [data-tiptap-editor-target='editor'] [contenteditable='true']")
      if (editable) {
        editable.focus()
        this._caretToStart(editable)
      }
    })
  }

  _focusSearch(fieldName) {
    requestAnimationFrame(() => {
      const hidden = this.element.querySelector(`input[name="${fieldName}"]`)
      const search = hidden?.closest("[data-controller~='contact-pill-input']")
        ?.querySelector("[data-contact-pill-input-target='search']")
      search?.focus()
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

  _showSubmitting(button) {
    const btn = button || (this.hasSendButtonTarget ? this.sendButtonTarget : null)
    if (!btn) return
    this._pendingButton = btn
    btn.dataset.originalHtml = btn.innerHTML
    const label = btn.textContent.trim()
    btn.setAttribute("disabled", "disabled")
    btn.classList.add("opacity-60")
    btn.innerHTML = `<svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
      </svg> ${label}`
  }

  _editorController() {
    const el = this.element.querySelector("[data-controller~='tiptap-editor']")
    return el && this.application.getControllerForElementAndIdentifier(el, "tiptap-editor")
  }

  _autosaveController() {
    return this.application.getControllerForElementAndIdentifier(this.element, "compose-autosave")
  }
}
