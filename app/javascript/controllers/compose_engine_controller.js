import { Controller } from "@hotwired/stimulus"

// The text content of an HTML fragment, via the parser (never a tag-stripping
// regex): used only to ask "is there any text here?".
function textOf(html) {
  if (!html) return ""
  return new DOMParser().parseFromString(html, "text/html").body.textContent.trim()
}

// Shell-agnostic behavior of the composer engine (Campbooks::Compose::Engine):
// envelope collapse/expand, Cc/Bcc reveal, quoted-thread expansion, submit
// validation + busy state, ⌘↵ send, focus-on-open, and discard. Draft
// persistence lives in the sibling compose-autosave controller on the same
// <form>; shells listen for the bubbled `compose-engine:closed` event.
export default class extends Controller {
  static targets = [
    "summary", "summaryRecipients", "summarySubject", "fields",
    "ccRow", "bccRow", "ccToggle", "bccToggle",
    "subjectInput", "subjectInferred", "collapseButton", "quoteWrap", "quotedInput", "sendButton",
    "scoutDraft", "scoutText", "scoutSpark", "scoutChip"
  ]
  static values = {
    messageId: { type: String, default: "" },
    rewriteUrl: { type: String, default: "" },
    rewriteDoneText: { type: String, default: "Rewrote your draft" },
    rewriteFailedText: { type: String, default: "Could not rewrite the draft" },
    undoRewriteText: { type: String, default: "Undo rewrite" }
  }

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

  changedAnywhere(event) {
    this._syncCollapseButton()
    this._maybeClearScoutMark(event)
  }

  // ── bold editor footer (formatting delegates + tone rewrite) ──
  // The bold composer's footer buttons sit outside the tiptap-editor element, so
  // they route through here to the resolved controller.
  formatBold()   { this._editorController()?.toggleBold() }
  formatItalic() { this._editorController()?.toggleItalic() }
  formatLink(event) { this._editorController()?.openLink(event) }

  attach(event) {
    event?.preventDefault()
    this.element.querySelector("[data-compose-attachments-target='fileInput']")?.click()
  }

  // The subject was inferred from context; editing it makes it the writer's own.
  clearSubjectInferred() {
    if (this.hasSubjectInferredTarget) this.subjectInferredTarget.remove()
  }

  // ── Scout's-draft mark (bold) ────────────────────────────────
  // Set when Scout's intent draft lands in the editor (compose-chat:body-set);
  // the chip clears the moment the writer edits the body — then it's theirs.
  markScoutDraft() {
    this.element.dataset.scoutDraft = "true"
    if (this.hasScoutChipTarget) this.scoutChipTarget.classList.remove("hidden")
  }

  _maybeClearScoutMark(event) {
    if (this.element.dataset.scoutDraft !== "true") return
    const target = event?.target
    const fromEditor = target && (target.name === "body" || target.closest?.(".ProseMirror"))
    if (!fromEditor) return
    this.element.dataset.scoutDraft = "false"
    if (this.hasScoutChipTarget) this.scoutChipTarget.classList.add("hidden")
  }

  // Rewrite the current body to a tone (Shorter / Warmer / Firmer). Replaces the
  // editor content and offers a one-step "Undo rewrite" toast.
  rewriteDraft(event) {
    event?.preventDefault()
    const tone = event.params?.tone
    const editor = this._editorController()
    if (!tone || !editor) return

    const previous = editor.getHTML()
    if (!this._hasText(previous)) return

    const button = event.currentTarget
    button?.setAttribute("disabled", "disabled")
    button?.classList.add("opacity-60")

    fetch(this.rewriteUrlValue || "/email_messages/rewrite_draft", {
      method: "POST",
      headers: {
        "X-CSRF-Token": document.querySelector("meta[name='csrf-token']")?.content || "",
        "Accept": "application/json",
        "Content-Type": "application/json"
      },
      body: JSON.stringify({ body: previous, tone })
    })
      .then((r) => (r.ok ? r.json() : Promise.reject(r)))
      .then((data) => {
        if (!data?.body) return
        editor.setContent(data.body) // _sync fires a bubbling input → autosave
        this._rewriteToast(editor, previous)
      })
      .catch(() => this._toast(this.rewriteFailedTextValue, "warning"))
      .finally(() => {
        button?.removeAttribute("disabled")
        button?.classList.remove("opacity-60")
      })
  }

  _hasText(html) {
    return Boolean(textOf(html))
  }

  _rewriteToast(editor, previousHtml) {
    const toast = this._toast(this.rewriteDoneTextValue, "success", { undo: true })
    if (!toast) return
    toast.querySelector("[data-undo]")?.addEventListener("click", () => {
      editor.setContent(previousHtml)
      toast.remove()
    })
  }

  // Minimal client-side toast into the shared #action_toasts region (mirrors
  // Campbooks::ActionToast's capsule + the action-toast auto-dismiss).
  _toast(message, variant = "success", { undo = false } = {}) {
    const region = document.getElementById("action_toasts")
    if (!region) return null
    const toast = document.createElement("div")
    toast.className =
      "pointer-events-auto inline-flex max-w-full items-center gap-2.5 rounded-full border border-border " +
      "bg-card/95 py-1.5 pl-3 pr-3 text-sm font-medium text-foreground shadow-lg backdrop-blur animate-fade-in"
    toast.setAttribute("role", "status")
    toast.dataset.actionToastDuration = undo ? "7000" : "4000"

    const label = document.createElement("span")
    label.className = "min-w-0"
    label.textContent = message
    toast.appendChild(label)

    if (undo) {
      const button = document.createElement("button")
      button.type = "button"
      button.dataset.undo = ""
      button.className =
        "-my-0.5 flex-shrink-0 cursor-pointer rounded-full px-2.5 py-1 text-sm font-semibold text-primary " +
        "transition-colors hover:bg-primary/10"
      button.textContent = this.undoRewriteTextValue
      toast.appendChild(button)
    }

    region.appendChild(toast)
    return toast
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
