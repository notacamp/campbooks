import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  connect() {
    // Set up auto-dismiss for any toasts already in the DOM (from flash on page load)
    this.element.querySelectorAll("[data-action-toast-duration]").forEach((el) => {
      this.#scheduleDismiss(el)
    })

    // Watch for new toasts added via Turbo Stream
    this.observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        mutation.addedNodes.forEach((node) => {
          if (node.nodeType === 1 && node.hasAttribute?.("data-action-toast-duration")) {
            this.#scheduleDismiss(node)
          }
        })
      })
    })
    this.observer.observe(this.element, { childList: true })
  }

  disconnect() {
    this.observer?.disconnect()
  }

  dismiss(event) {
    const toast = event.target.closest("[data-action-toast-duration]")
    if (toast) this.#remove(toast)
  }

  // private

  #scheduleDismiss(el) {
    const ms = parseInt(el.dataset.actionToastDuration, 10) || 4000
    setTimeout(() => this.#remove(el), ms)
  }

  #remove(el) {
    el.style.transition = "opacity 200ms ease-out, transform 200ms ease-out"
    el.style.opacity = "0"
    el.style.transform = "translateY(4px)"
    setTimeout(() => el.remove(), 200)
  }
}
