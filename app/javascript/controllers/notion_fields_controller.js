import { Controller } from "@hotwired/stimulus"

// Loads the per-field config form for a chosen Notion database into a container.
// Reads the current workspace + database values from sibling inputs and fetches
// WorkflowsController#notion_fields, which renders one Liquid input per database
// property. Keeps the workflow builder a single form (no nested form / turbo-frame).
export default class extends Controller {
  static targets = ["integration", "database", "fields"]
  static values = { url: String, prefix: String, current: Object, loading: String, error: String }

  connect() {
    // Re-editing a step that already has a database → show its fields immediately.
    if (this.hasDatabaseTarget && this.databaseTarget.value.trim() !== "") this.load()
  }

  async load(event) {
    if (event) event.preventDefault()

    const databaseId = this.hasDatabaseTarget ? this.databaseTarget.value.trim() : ""
    if (databaseId === "") return

    const params = new URLSearchParams({
      integration_id: this.hasIntegrationTarget ? this.integrationTarget.value : "",
      database_id: databaseId,
      prefix: this.prefixValue
    })
    if (this.hasCurrentValue && Object.keys(this.currentValue).length > 0) {
      params.set("values", JSON.stringify(this.currentValue))
    }

    this.fieldsTarget.innerHTML = `<p class="text-sm text-muted-foreground">${this.loadingValue}</p>`
    try {
      const response = await fetch(`${this.urlValue}?${params.toString()}`, {
        headers: { Accept: "text/html", "X-Requested-With": "XMLHttpRequest" }
      })
      this.fieldsTarget.innerHTML = await response.text()
    } catch (_error) {
      this.fieldsTarget.innerHTML = `<p class="text-sm text-red-600">${this.errorValue}</p>`
    }
  }
}
