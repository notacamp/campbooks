import { Controller } from "@hotwired/stimulus"

// Drives the "add a step" picker modal: any "+" connector opens it, the search
// box filters the cards live, and arrow keys / Enter / Escape make it keyboard
// friendly. Each card is its own POST form, so selecting one just submits.
export default class extends Controller {
  static targets = ["modal", "search", "card", "group", "empty"]

  open(event) {
    if (event) event.preventDefault()
    this.modalTarget.classList.remove("hidden")
    document.body.style.overflow = "hidden"
    this.searchTarget.value = ""
    this.filter()
    this.searchTarget.focus()
  }

  close() {
    this.modalTarget.classList.add("hidden")
    document.body.style.overflow = ""
  }

  backdropClose(event) {
    if (event.target === this.modalTarget) this.close()
  }

  filter() {
    const query = this.searchTarget.value.trim().toLowerCase()

    this.cardTargets.forEach((card) => {
      const match = !query || card.dataset.keywords.includes(query)
      card.closest("form").classList.toggle("hidden", !match)
    })

    this.groupTargets.forEach((group) => {
      const anyVisible = [...group.querySelectorAll("form")].some((f) => !f.classList.contains("hidden"))
      group.classList.toggle("hidden", !anyVisible)
    })

    const visible = this.visibleCards()
    if (this.hasEmptyTarget) this.emptyTarget.classList.toggle("hidden", visible.length > 0)
    this.setActive(0)
  }

  keydown(event) {
    switch (event.key) {
      case "Escape":
        event.preventDefault()
        this.close()
        break
      case "ArrowDown":
        event.preventDefault()
        this.move(1)
        break
      case "ArrowUp":
        event.preventDefault()
        this.move(-1)
        break
      case "Enter": {
        event.preventDefault()
        const card = this.visibleCards()[this.activeIndex]
        if (card) card.click()
        break
      }
    }
  }

  activate(event) {
    this.setActive(this.visibleCards().indexOf(event.currentTarget))
  }

  // --- helpers ---

  visibleCards() {
    return this.cardTargets.filter((c) => !c.closest("form").classList.contains("hidden"))
  }

  move(delta) {
    const count = this.visibleCards().length
    if (!count) return
    this.setActive((this.activeIndex + delta + count) % count)
    this.visibleCards()[this.activeIndex]?.scrollIntoView({ block: "nearest" })
  }

  setActive(index) {
    this.activeIndex = index
    this.cardTargets.forEach((c) => c.removeAttribute("data-active"))
    const card = this.visibleCards()[index]
    if (card) card.setAttribute("data-active", "true")
  }
}
