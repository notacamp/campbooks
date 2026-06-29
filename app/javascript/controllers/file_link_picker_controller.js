import { Controller } from "@hotwired/stimulus"

// Compose "Insert file link" (Files Phase 3b). Opens a modal of the workspace's
// files (lazy turbo-frame); on pick, POSTs to mint/reuse a public FileShareLink and
// inserts an <a> into the sibling tiptap-editor's body via its appendContent API.
export default class extends Controller {
  static targets = ["dialog"]

  open(event) {
    event?.preventDefault()
    this.dialogTarget.showModal()
  }

  close() {
    this.dialogTarget.close()
  }

  backdrop(event) {
    if (event.target === this.dialogTarget) this.close()
  }

  async pick(event) {
    event.preventDefault()
    const btn = event.currentTarget
    if (btn.dataset.busy) return
    btn.dataset.busy = "1"
    try {
      const res = await fetch("/files/public_links", {
        method: "POST",
        headers: { "X-CSRF-Token": this.#csrf(), "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify({ shareable_type: btn.dataset.shareableType, shareable_id: btn.dataset.shareableId })
      })
      const data = await res.json().catch(() => ({}))
      if (res.ok && data.url) {
        this.#insert(data.url, data.name || btn.dataset.name)
        this.close()
      }
    } finally {
      delete btn.dataset.busy
    }
  }

  #insert(url, name) {
    const editorEl = this.element.closest("form")?.querySelector("[data-controller~='tiptap-editor']")
    const ctrl = editorEl && this.application.getControllerForElementAndIdentifier(editorEl, "tiptap-editor")
    if (!ctrl) return
    const label = this.#escape(name || url)
    ctrl.appendContent(`<a href="${url}" target="_blank" rel="noopener noreferrer nofollow">${label}</a>&nbsp;`)
  }

  #escape(text) {
    return String(text).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c])
  }

  #csrf() {
    return document.querySelector("meta[name='csrf-token']")?.content || ""
  }
}
