import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["step", "title", "progress", "bar", "nextBtn", "backBtn", "submitBtn", "stepField"]
  static values = { current: { type: Number, default: 0 } }

  connect() {
    if (this.hasStepFieldTarget) {
      const initial = parseInt(this.stepFieldTarget.value, 10)
      if (!isNaN(initial)) this.currentValue = initial
    }
    this.showStep(this.currentValue)
    this.element.addEventListener("keydown", this.#onKeydown.bind(this))
  }

  disconnect() {
    this.element.removeEventListener("keydown", this.#onKeydown.bind(this))
  }

  next() {
    if (this.currentValue < this.stepTargets.length - 1) {
      this.showStep(this.currentValue + 1)
    }
  }

  back() {
    if (this.currentValue > 0) {
      this.currentValue--
      this.showStep(this.currentValue)
    }
  }

  showStep(index) {
    this.currentValue = index

    this.stepTargets.forEach((step, i) => {
      if (i === index) {
        step.classList.remove("hidden")
        step.classList.add("animate-fade-in")
        this.#focusInput(step)
      } else {
        step.classList.add("hidden")
        step.classList.remove("animate-fade-in")
      }
    })

    const currentStep = this.stepTargets[index]
    const title = currentStep.dataset.stepTitle
    if (title && this.hasTitleTarget) {
      this.titleTarget.textContent = title
    }

    const total = this.stepTargets.length
    if (this.hasProgressTarget) {
      this.progressTarget.textContent = `Step ${index + 1} of ${total}`
    }
    if (this.hasBarTarget) {
      this.barTarget.style.transform = `scaleX(${(index + 1) / total})`
    }

    const isLast = index === total - 1
    if (this.hasBackBtnTarget) this.#setVisible(this.backBtnTarget, index > 0)
    if (this.hasNextBtnTarget) this.#setVisible(this.nextBtnTarget, !isLast)
    if (this.hasSubmitBtnTarget) this.#setVisible(this.submitBtnTarget, isLast)
  }

  // Inline display overrides Tailwind's .hidden — Campbooks::Button sets its own
  // display utility, which would otherwise win the cascade and leave a stray
  // button visible on the last step. Keep the class in sync for semantics.
  #setVisible(el, visible) {
    el.classList.toggle("hidden", !visible)
    el.style.display = visible ? "" : "none"
  }

  #focusInput(step) {
    const input = step.querySelector("input, textarea, select")
    if (input) {
      requestAnimationFrame(() => {
        input.focus()
        input.scrollIntoView({ block: "center" })
      })
    }
  }

  #onKeydown(event) {
    if (event.key === "Enter" && event.target.tagName !== "TEXTAREA") {
      const isLast = this.currentValue === this.stepTargets.length - 1
      if (!isLast) {
        event.preventDefault()
        this.next()
      }
    }
  }
}
