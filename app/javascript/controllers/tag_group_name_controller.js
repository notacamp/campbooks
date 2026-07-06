import { Controller } from "@hotwired/stimulus"

// Prefills the group-name field from the FIRST tag added while creating a
// group; later pills never overwrite it. A name the user typed is left alone
// entirely (clearing the field re-arms the prefill), and unchecking the pill
// that named the group falls back to the first remaining selection.
export default class extends Controller {
  static targets = ["name"]

  typed() {
    // The user took over — a nonblank manual value stops being "auto".
    this.autoValue = null
  }

  toggle(event) {
    if (event.target.type !== "checkbox") return

    const name = this.nameTarget
    const auto = name.value.trim() === "" || name.value === this.autoValue
    if (!auto) return

    if (event.target.checked) {
      // Fill only from the first selection; keep the first-added name after.
      if (name.value.trim() === "") {
        name.value = this.labelFor(event.target)
        this.autoValue = name.value
      }
    } else if (name.value === this.labelFor(event.target)) {
      const first = this.element.querySelector("input[type=checkbox]:checked")
      name.value = first ? this.labelFor(first) : ""
      this.autoValue = name.value || null
    }
  }

  labelFor(checkbox) {
    const label = checkbox.closest("label")
    return label ? label.innerText.trim() : ""
  }
}
