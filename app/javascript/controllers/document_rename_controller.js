import { Controller } from "@hotwired/stimulus"

// Inline rename for a document row in the documents list. Clicking the pencil swaps
// the name link for a text input; Enter or blur saves (PATCH /documents/:id/rename,
// fire-and-forget JSON), Escape cancels. The name is the display title (stored in
// metadata) — blank clears it back to the entity/filename. The display link's href
// is untouched, so it keeps linking to the document.
export default class extends Controller {
  static targets = ["display", "input", "editButton"]
  static values = { url: String }

  edit(event) {
    event?.preventDefault()
    this.saving = false
    this.displayTarget.classList.add("hidden")
    if (this.hasEditButtonTarget) this.editButtonTarget.classList.add("hidden")
    this.inputTarget.classList.remove("hidden")
    this.inputTarget.focus()
    this.inputTarget.select()
  }

  cancel() {
    this.inputTarget.classList.add("hidden")
    this.displayTarget.classList.remove("hidden")
    if (this.hasEditButtonTarget) this.editButtonTarget.classList.remove("hidden")
  }

  // Enter and blur both land here; `saving` guards against the double-fire when Enter
  // is followed by the blur it triggers.
  save(event) {
    if (event?.type === "keydown") event.preventDefault()
    if (this.saving) return
    this.saving = true

    fetch(this.urlValue, {
      method: "PATCH",
      headers: { "X-CSRF-Token": this.csrf, "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify({ document: { title: this.inputTarget.value } })
    })
      .then((r) => (r.ok ? r.json() : Promise.reject(r)))
      .then((d) => { if (d?.display_title) this.displayTarget.textContent = d.display_title })
      .catch(() => {})
      .finally(() => this.cancel())
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }
}
