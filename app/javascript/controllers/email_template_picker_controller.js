import { Controller } from "@hotwired/stimulus"

// Drives the composer's "Use template" picker. Opening loads the template list
// into a turbo-frame (carrying the current recipient for prefill); picking a
// template loads its variables form; "Insert" POSTs the variables to the apply
// endpoint and folds the rendered subject/body + document-template PDF
// attachments back into the surrounding compose <form>.
export default class extends Controller {
  static targets = ["modal", "frame"]
  static values = { listUrl: String }

  open(event) {
    event?.preventDefault()
    const url = new URL(this.listUrlValue, window.location.origin)
    const to = this._toAddress()
    if (to) url.searchParams.set("to_address", to)
    // Always (re)set src so reopening refreshes the list.
    this.frameTarget.setAttribute("src", url.pathname + url.search)
    this.modalTarget.classList.remove("hidden")
  }

  close(event) {
    event?.preventDefault()
    this.modalTarget.classList.add("hidden")
  }

  backdropClose(event) {
    if (event.target === this.modalTarget) this.close(event)
  }

  keydown(event) {
    if (event.key === "Escape") this.close(event)
  }

  // Insert button in the loaded fill form:
  // data-action="email-template-picker#apply" data-email-template-picker-url-param="/email_templates/:id/apply"
  async apply(event) {
    event.preventDefault()
    const url = event.params.url
    if (!url) return

    const button = event.currentTarget
    button.disabled = true

    const variables = {}
    this.frameTarget.querySelectorAll("[data-template-var]").forEach((el) => {
      variables[el.dataset.templateVar] = el.value
    })

    try {
      const res = await fetch(url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Accept": "application/json",
          "X-CSRF-Token": this._csrf()
        },
        body: JSON.stringify({ variables })
      })
      const data = await res.json().catch(() => ({}))
      if (!res.ok) {
        button.disabled = false
        return
      }
      this._inject(data)
      this.close()
    } catch (_e) {
      button.disabled = false
    }
  }

  // --- internals ----------------------------------------------------------

  _inject(data) {
    const form = this.element.closest("form")
    if (!form) return

    const subject = form.querySelector("input[name='subject']")
    if (subject && data.subject != null) {
      subject.value = data.subject
      subject.dispatchEvent(new Event("input", { bubbles: true }))
    }

    this._setBody(form, data.body_html)

    const tray = form.querySelector("[data-compose-attachments-target='tray']")
    if (tray) (data.attachments || []).forEach((att) => tray.appendChild(this._chip(att)))

    // Stash for the scheduling path so the send job can re-render per occurrence.
    this._stash(form, "email_template_id", data.email_template_id)
    this._stash(form, "template_context", JSON.stringify(data.variables || {}))
  }

  _setBody(form, html) {
    if (html == null) return
    const el = form.querySelector("[data-controller~='tiptap-editor']")
    if (!el) return
    const tc = this.application.getControllerForElementAndIdentifier(el, "tiptap-editor")
    if (!tc) return
    const current = (tc.getHTML?.() || "").replace(/<p>\s*<\/p>/g, "").trim()
    if (current === "" && tc.setContent) tc.setContent(html)
    else if (tc.appendContent) tc.appendContent(html)
    else if (tc.setContent) tc.setContent(html)
  }

  _chip(att) {
    const chip = document.createElement("span")
    chip.className = "attachment-chip"

    const label = document.createElement("span")
    label.className = "attachment-chip-name"
    label.textContent = att.size ? `${att.filename} · ${this._humanSize(att.size)}` : att.filename

    const remove = document.createElement("button")
    remove.type = "button"
    remove.setAttribute("aria-label", "Remove")
    remove.textContent = "✕"
    remove.addEventListener("click", () => chip.remove())

    const input = document.createElement("input")
    input.type = "hidden"
    input.name = "attachments[]"
    input.value = att.signed_id

    chip.append(label, remove, input)
    return chip
  }

  _stash(form, name, value) {
    let input = form.querySelector(`input[type='hidden'][name='${name}']`)
    if (!input) {
      input = document.createElement("input")
      input.type = "hidden"
      input.name = name
      form.appendChild(input)
    }
    input.value = value == null ? "" : value
  }

  _toAddress() {
    const form = this.element.closest("form")
    const el = form?.querySelector("[name='to_address']")
    return el ? el.value : ""
  }

  _humanSize(bytes) {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  _csrf() {
    return document.querySelector("meta[name='csrf-token']")?.content || ""
  }
}
