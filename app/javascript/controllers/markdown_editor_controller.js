import { Controller } from "@hotwired/stimulus"
import { marked } from "marked"

export default class extends Controller {
  static targets = ["textarea", "preview"]
  static values = { rows: { type: Number, default: 3 } }

  connect() {
    this.textareaTarget.rows = this.rowsValue
    this.textareaTarget.classList.add("markdown-editor__input")
    this.showWrite()
  }

  showWrite() {
    this.textareaTarget.classList.remove("hidden")
    if (this.hasPreviewTarget) this.previewTarget.classList.add("hidden")
    this.element.querySelector("[data-md-tab='write']")?.classList.add("border-accent-600", "text-accent-600", "bg-accent-50/50")
    this.element.querySelector("[data-md-tab='write']")?.classList.remove("border-transparent", "text-gray-500")
    this.element.querySelector("[data-md-tab='preview']")?.classList.remove("border-accent-600", "text-accent-600", "bg-accent-50/50")
    this.element.querySelector("[data-md-tab='preview']")?.classList.add("border-transparent", "text-gray-500")
  }

  showPreview() {
    const md = this.textareaTarget.value || "*Nothing written yet.*"
    this.previewTarget.innerHTML = marked.parse(md)
    this.textareaTarget.classList.add("hidden")
    this.previewTarget.classList.remove("hidden")
    this.element.querySelector("[data-md-tab='preview']")?.classList.add("border-accent-600", "text-accent-600", "bg-accent-50/50")
    this.element.querySelector("[data-md-tab='preview']")?.classList.remove("border-transparent", "text-gray-500")
    this.element.querySelector("[data-md-tab='write']")?.classList.remove("border-accent-600", "text-accent-600", "bg-accent-50/50")
    this.element.querySelector("[data-md-tab='write']")?.classList.add("border-transparent", "text-gray-500")
  }
}
