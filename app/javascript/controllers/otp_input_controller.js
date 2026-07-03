import { Controller } from "@hotwired/stimulus"

// Segmented one-time-code entry: six single-digit boxes that behave like one
// field. Typing advances, backspace retreats, pasting a whole code fills every
// box, and the sixth digit submits the form. The real value lives in a hidden
// input (target "code") so the server keeps reading params[:code] unchanged.
export default class extends Controller {
  static targets = ["box", "code"]

  connect() {
    this.boxTargets[0]?.focus()
  }

  input(event) {
    const box = event.target
    box.value = box.value.replace(/\D/g, "").slice(-1)
    if (box.value) this.boxAfter(box)?.focus()
    this.sync()
  }

  keydown(event) {
    const box = event.target
    if (event.key === "Backspace" && !box.value) {
      const previous = this.boxBefore(box)
      if (previous) { previous.focus(); previous.value = ""; this.sync() }
    } else if (event.key === "ArrowLeft") {
      this.boxBefore(box)?.focus()
    } else if (event.key === "ArrowRight") {
      this.boxAfter(box)?.focus()
    }
  }

  paste(event) {
    event.preventDefault()
    const digits = (event.clipboardData?.getData("text") || "").replace(/\D/g, "").slice(0, this.boxTargets.length)
    if (!digits) return
    this.boxTargets.forEach((box, i) => { box.value = digits[i] || "" })
    this.boxTargets[Math.min(digits.length, this.boxTargets.length - 1)]?.focus()
    this.sync()
  }

  sync() {
    const code = this.boxTargets.map((box) => box.value).join("")
    this.codeTarget.value = code
    if (code.length === this.boxTargets.length) this.element.requestSubmit()
  }

  boxAfter(box) { return this.boxTargets[this.boxTargets.indexOf(box) + 1] }
  boxBefore(box) { return this.boxTargets[this.boxTargets.indexOf(box) - 1] }
}
