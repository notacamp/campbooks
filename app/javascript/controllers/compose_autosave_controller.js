import { Controller } from "@hotwired/stimulus"

// Persists the open composer into a DraftEmail row so nothing typed is ever
// lost. Attached to the compose <form> in both shells (Dock + Desk).
//
// Contract: the row is created on the FIRST user input (an opened-and-abandoned
// reply never becomes a draft), then PATCHed on a debounce. Sending suspends
// autosave (the server destroys the draft after a successful send; a failed
// send re-arms on the next keystroke). Leaving the page flushes with a
// keepalive fetch. Discard is explicit and deletes the row.
export default class extends Controller {
  static targets = ["status", "draftIdInput"]
  static values = {
    url: String,                       // /draft_emails
    draftId: { type: String, default: "" },
    mode: { type: String, default: "new_message" },
    inReplyToId: { type: String, default: "" },
    savingText: { type: String, default: "Saving…" },
    savedText: { type: String, default: "Draft saved" }
  }

  static DEBOUNCE_MS = 1500

  connect() {
    this._timer = null
    this._suspended = false
    this._dirty = false
    this._creating = false

    this._flushBound = this._flush.bind(this)
    document.addEventListener("turbo:before-visit", this._flushBound)
    window.addEventListener("pagehide", this._flushBound)
  }

  disconnect() {
    clearTimeout(this._timer)
    document.removeEventListener("turbo:before-visit", this._flushBound)
    window.removeEventListener("pagehide", this._flushBound)
  }

  // Wired as `input->compose-autosave#changed` on the form element.
  changed() {
    this._suspended = false
    this._dirty = true
    clearTimeout(this._timer)
    this._timer = setTimeout(() => this._save(), this.constructor.DEBOUNCE_MS)
  }

  // Wired to the form's submit (send / schedule): the server consumes the
  // draft on success, so autosave must stand down instead of resurrecting it.
  suspend() {
    this._suspended = true
    this._dirty = false
    clearTimeout(this._timer)
  }

  // Explicit discard from the shell. Deletes the row and stands down.
  async discard() {
    this.suspend()
    if (!this.draftIdValue) return
    try {
      await fetch(`${this.urlValue}/${this.draftIdValue}`, {
        method: "DELETE",
        headers: this._headers()
      })
    } catch (_e) { /* the prune cap cleans up stragglers */ }
    this._setDraftId("")
  }

  get draftId() {
    return this.draftIdValue
  }

  // ── internals ─────────────────────────────────────────────────
  async _save({ keepalive = false } = {}) {
    if (this._suspended || !this._dirty) return
    if (this._creating) { this.changed(); return }

    this._dirty = false
    this._setStatus(this.savingTextValue)

    const body = JSON.stringify({ draft_email: this._payload() })
    const create = !this.draftIdValue
    if (create) this._creating = true

    try {
      const res = await fetch(create ? this.urlValue : `${this.urlValue}/${this.draftIdValue}`, {
        method: create ? "POST" : "PATCH",
        headers: this._headers(),
        body,
        keepalive
      })
      if (res.status === 404 && !create) {
        // Pruned or deleted elsewhere — recreate on the next change.
        this._setDraftId("")
        this._dirty = true
        return
      }
      const data = await res.json().catch(() => ({}))
      if (res.ok && data.id) {
        this._setDraftId(data.id)
        this._setStatus(this.savedTextValue)
      }
    } catch (_e) {
      this._dirty = true // retry on the next change
    } finally {
      this._creating = false
    }
  }

  _flush() {
    if (this._suspended || !this._dirty) return
    clearTimeout(this._timer)
    this._save({ keepalive: true })
  }

  _payload() {
    const payload = {
      mode: this.modeValue,
      to_address: this._field("to_address"),
      cc_address: this._field("cc_address"),
      bcc_address: this._field("bcc_address"),
      subject: this._field("subject"),
      body: this._field("body"),
      signature_id: this._field("signature_id"),
      email_account_id: this._field("email_account_id"),
      attachments_json: this._attachments()
    }
    if (this.inReplyToIdValue) payload.in_reply_to_id = this.inReplyToIdValue
    return payload
  }

  _field(name) {
    return this.element.querySelector(`[name="${name}"]`)?.value ?? ""
  }

  _attachments() {
    return Array.from(this.element.querySelectorAll(".attachment-chip")).flatMap((chip) => {
      const signedId = chip.querySelector("input[type='hidden']")?.value
      if (!signedId) return []
      return [{
        signed_id: signedId,
        filename: chip.dataset.filename || "",
        byte_size: parseInt(chip.dataset.byteSize || "0", 10)
      }]
    })
  }

  _setDraftId(id) {
    this.draftIdValue = id
    if (this.hasDraftIdInputTarget) this.draftIdInputTarget.value = id
  }

  _setStatus(text) {
    if (this.hasStatusTarget) this.statusTarget.textContent = text
  }

  _headers() {
    return {
      "Content-Type": "application/json",
      "Accept": "application/json",
      "X-CSRF-Token": document.querySelector("meta[name='csrf-token']")?.content || ""
    }
  }
}
