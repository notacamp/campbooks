import { Controller } from "@hotwired/stimulus"

// The People conversation scroll container. Opens scrolled to the bottom (the
// newest message sits next to the reply box, oldest at the top), matching the
// mock. Only anchors on connect — an "Earlier" prepend leaves your scroll where
// it is so you keep reading older messages.
export default class extends Controller {
  connect() {
    // Wait a frame so images/bodies have laid out before we measure height.
    requestAnimationFrame(() => { this.element.scrollTop = this.element.scrollHeight })
  }
}
