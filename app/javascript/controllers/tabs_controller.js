import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["tab", "panel"]

  select(event) {
    const button = event.currentTarget
    const panelId = button.dataset.tabsPanel

    this.tabTargets.forEach(tab => {
      tab.classList.toggle("text-accent-600", tab === button)
      tab.classList.toggle("border-accent-600", tab === button)
      tab.classList.toggle("bg-accent-50/50", tab === button)
      tab.classList.toggle("rounded-tl-xl", tab === this.tabTargets[0])
      tab.classList.toggle("rounded-tr-xl", tab === this.tabTargets[this.tabTargets.length - 1])
      tab.classList.toggle("text-gray-500", tab !== button)
      tab.classList.toggle("border-transparent", tab !== button)
      tab.setAttribute("aria-selected", tab === button ? "true" : "false")
    })

    this.panelTargets.forEach(panel => {
      panel.classList.toggle("hidden", panel.id !== panelId)
    })
  }
}
