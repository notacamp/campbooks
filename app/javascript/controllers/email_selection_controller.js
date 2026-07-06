import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["toolbar", "header", "count", "checkbox", "groupCheckbox", "sectionToggle", "selectToggle", "tagMenu", "folderMenu", "snoozeMenu"]

  connect() {
    this.selected = new Set()
    // Map<groupName, collapsedCount> — groups selected via their row checkbox.
    this.selectedGroups = new Map()
    this.selectMode = this.element.dataset.selectMode === "on"
  }

  // --- Selection ---

  toggle(event) {
    const cb = event.target
    if (cb.checked) {
      this.selected.add(cb.value)
    } else {
      this.selected.delete(cb.value)
    }
    this.updateUI()
  }

  // Toggle a collapsed group row checkbox. The group name and its collapsed
  // count come from data attributes on the checkbox; the count is summed into
  // the toolbar counter so the user can see how many emails will be acted on.
  toggleGroup(event) {
    const cb = event.target
    const name = cb.dataset.groupName
    const count = parseInt(cb.dataset.groupCount || "0", 10)
    if (cb.checked) {
      this.selectedGroups.set(name, count)
    } else {
      this.selectedGroups.delete(name)
    }
    this.updateUI()
  }

  toggleAll(event) {
    const checked = event.target.checked
    this.checkboxTargets.forEach(cb => {
      cb.checked = checked
      if (checked) {
        this.selected.add(cb.value)
      } else {
        this.selected.delete(cb.value)
      }
    })
    this.groupCheckboxTargets.forEach(gcb => {
      gcb.checked = checked
      const name = gcb.dataset.groupName
      const count = parseInt(gcb.dataset.groupCount || "0", 10)
      if (checked) {
        this.selectedGroups.set(name, count)
      } else {
        this.selectedGroups.delete(name)
      }
    })
    this.updateUI()
  }

  // Select/deselect every row in one date section (Today / This Week / Priority …).
  // Group rows are not part of any date section so they are unaffected here.
  toggleSection(event) {
    const checked = event.target.checked
    this.sectionCheckboxes(event.target.dataset.section).forEach(cb => {
      cb.checked = checked
      if (checked) {
        this.selected.add(cb.value)
      } else {
        this.selected.delete(cb.value)
      }
    })
    this.updateUI()
  }

  clear() {
    this.selected.clear()
    this.selectedGroups.clear()
    this.checkboxTargets.forEach(cb => cb.checked = false)
    this.groupCheckboxTargets.forEach(gcb => gcb.checked = false)
    const selectAll = this.element.querySelector("[data-email-selection-select-all]")
    if (selectAll) selectAll.checked = false
    this.updateUI()
    this.closeDropdowns()
  }

  // --- Select mode (persistent checkboxes + tap-to-select, works on touch) ---

  toggleMode() {
    this.setMode(!this.selectMode)
  }

  setMode(on) {
    this.selectMode = on
    this.element.dataset.selectMode = on ? "on" : "off"
    if (this.hasSelectToggleTarget) {
      this.selectToggleTarget.setAttribute("aria-pressed", on ? "true" : "false")
    }
    // Leaving select mode drops any pending selection.
    if (!on) this.clear()
  }

  // In select mode, a tap anywhere on a row toggles its checkbox instead of
  // opening the email — so selection works without a hover (touch). Delegated
  // from the controller root; taps on the checkbox, pin, or other controls
  // (which aren't inside the row's <a>) fall through to their normal behavior.
  rowClick(event) {
    if (!this.selectMode) return
    const link = event.target.closest("a[href]")
    if (!link) return
    const item = link.closest("[id^='thread_item']")
    if (!item || !this.element.contains(item)) return
    const cb = item.querySelector("input[data-email-selection-target='checkbox']")
    if (!cb) return

    event.preventDefault()
    cb.checked = !cb.checked
    if (cb.checked) {
      this.selected.add(cb.value)
    } else {
      this.selected.delete(cb.value)
    }
    this.updateUI()
  }

  // --- Actions ---

  bulkAction(event) {
    const button = event.currentTarget
    const tool = button.dataset.tool
    if (!tool) return

    if (tool === "delete") {
      if (!confirm(`Delete ${this.totalCount()} email thread(s)? This cannot be undone.`)) return
    }

    const body = new FormData()
    body.append("tool", tool)
    this.selected.forEach(id => body.append("email_ids[]", id))
    this.selectedGroups.forEach((_, name) => body.append("groups[]", name))

    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    fetch("/email_messages/bulk", {
      method: "POST",
      headers: {
        "X-CSRF-Token": csrfToken,
        "Accept": "text/vnd.turbo-stream.html"
      },
      body: body
    }).then(response => response.text()).then(html => {
      if (html) {
        Turbo.renderStreamMessage(html)
      }
    }).catch(() => {
      // Turbo Stream handles errors via the server response
    })

    this.clear()
  }

  bulkActionWithArgs(event) {
    const button = event.currentTarget
    const tool = button.dataset.tool
    const tagName = button.dataset.tagName
    const tagAction = button.dataset.tagAction
    const folderId = button.dataset.folderId
    const snoozedUntil = button.dataset.snoozedUntil

    if (!tool) return

    const body = new FormData()
    body.append("tool", tool)
    this.selected.forEach(id => body.append("email_ids[]", id))
    this.selectedGroups.forEach((_, name) => body.append("groups[]", name))
    if (tagName) body.append("tag_name", tagName)
    if (tagAction) body.append("tag_action", tagAction)
    if (folderId) body.append("folder_id", folderId)
    if (snoozedUntil) body.append("snoozed_until", snoozedUntil)

    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    fetch("/email_messages/bulk", {
      method: "POST",
      headers: {
        "X-CSRF-Token": csrfToken,
        "Accept": "text/vnd.turbo-stream.html"
      },
      body: body
    }).then(response => response.text()).then(html => {
      if (html) {
        Turbo.renderStreamMessage(html)
      }
    }).catch(() => {})

    this.clear()
    this.closeDropdowns()
  }

  // --- Dropdowns ---

  toggleDropdown(event) {
    const menu = event.currentTarget.dataset.menu
    this.closeDropdowns()
    if (menu === "tag") {
      this.tagMenuTarget.classList.toggle("hidden")
    } else if (menu === "folder") {
      this.folderMenuTarget.classList.toggle("hidden")
    } else if (menu === "snooze") {
      this.snoozeMenuTarget.classList.toggle("hidden")
    }
  }

  closeDropdowns() {
    if (this.hasTagMenuTarget) this.tagMenuTarget.classList.add("hidden")
    if (this.hasFolderMenuTarget) this.folderMenuTarget.classList.add("hidden")
    if (this.hasSnoozeMenuTarget) this.snoozeMenuTarget.classList.add("hidden")
  }

  // --- Keyboard ---

  keydown(event) {
    if (event.key === "Escape") {
      if (this.selectMode) {
        this.setMode(false)
      } else {
        this.clear()
      }
    }
  }

  // --- UI ---

  // Total count shown in the toolbar: individual message IDs + the collapsed
  // counts from every selected group row. This is the number of emails that
  // will be acted on, not the number of checkboxes checked.
  totalCount() {
    const groupTotal = [...this.selectedGroups.values()].reduce((sum, n) => sum + n, 0)
    return this.selected.size + groupTotal
  }

  updateUI() {
    const count = this.totalCount()
    if (count > 0) {
      this.toolbarTarget.classList.remove("hidden")
      if (this.hasHeaderTarget) this.headerTarget.classList.add("hidden")
      this.countTarget.textContent = `${count} selected`
    } else {
      this.toolbarTarget.classList.add("hidden")
      if (this.hasHeaderTarget) this.headerTarget.classList.remove("hidden")
    }
    this.syncSectionToggles()
  }

  // Reflect each section toggle as checked (all rows selected), indeterminate
  // (some), or unchecked (none) so it tracks individual row changes.
  syncSectionToggles() {
    this.sectionToggleTargets.forEach(toggle => {
      const rows = this.sectionCheckboxes(toggle.dataset.section)
      const checkedCount = rows.filter(cb => cb.checked).length
      toggle.checked = rows.length > 0 && checkedCount === rows.length
      toggle.indeterminate = checkedCount > 0 && checkedCount < rows.length
    })
  }

  sectionCheckboxes(section) {
    if (!section) return []
    return this.checkboxTargets.filter(cb => cb.dataset.section === section)
  }
}
