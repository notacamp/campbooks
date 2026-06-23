import { Controller } from "@hotwired/stimulus"

// Shows the configuration panel that matches the currently selected trigger or
// action type, hiding the others. Each panel declares the value it belongs to
// via data-workflow-type, e.g. data-workflow-type="slack_message".
export default class extends Controller {
  static targets = ["select", "panel"]

  connect() {
    this.update()
  }

  update() {
    const value = this.selectTarget.value
    this.panelTargets.forEach((panel) => {
      panel.classList.toggle("hidden", panel.dataset.workflowType !== value)
    })
  }
}
