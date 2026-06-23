# frozen_string_literal: true

# Campbooks::Swipeable wraps a row/card so it can be swiped/dragged to act. The
# colored action panels are hidden until you drag — in Lookbook the swipe-actions
# controller is live, so you can actually drag each row to preview the reveal,
# the two-stage color/icon morph, and the snap-back/commit. (Endpoints are "#"
# here, so a committed action just no-ops.)
class SwipeableComponentPreview < ViewComponent::Preview
  # Email row — swipe LEFT: Archive (blue) → Snooze (amber); swipe RIGHT: Trash
  # (orange) → Delete (red, confirm). Drag past ~half the width to morph to stage 2.
  def email_two_stage
    render(Campbooks::Swipeable.new(
      class: "bg-card border-b border-gray-100",
      left: [
        { key: "archive", label: "Archive", icon: :archive, color: "blue", endpoint: "#", params: { tool: "archive" } },
        { key: "snooze", label: "Snooze", icon: :snooze, color: "amber", endpoint: "#", params: { tool: "snooze" }, picker: "snooze" }
      ],
      right: [
        { key: "trash", label: "Trash", icon: :trash, color: "orange", endpoint: "#", params: { tool: "trash" } },
        { key: "delete", label: "Delete", icon: :delete, color: "red", endpoint: "#", params: { tool: "delete" },
          confirm: { title: "Delete permanently?", message: "This can't be undone." } }
      ]
    )) { demo_row("Quarterly report — Acme Inc.", "finance@acme.com") }
  end

  # Feed card — swipe LEFT: Dismiss (slate), single stage.
  def feed_single
    render(Campbooks::Swipeable.new(
      surface: "var(--color-background)",
      left: [ { key: "dismiss", label: "Dismiss", icon: :dismiss, color: "neutral", endpoint: "#", params: {} } ]
    )) { demo_card("Reply reminder", "You haven't replied to Dana in 3 days.") }
  end

  private

  def demo_row(title, subtitle)
    <<~HTML.html_safe
      <div class="flex items-center gap-3 px-3 py-3">
        <div class="h-9 w-9 flex-shrink-0 rounded-full bg-gray-200"></div>
        <div class="min-w-0 flex-1">
          <div class="truncate text-[13px] font-semibold text-gray-900">#{title}</div>
          <div class="truncate text-[11px] text-gray-500">#{subtitle}</div>
        </div>
      </div>
    HTML
  end

  def demo_card(title, subtitle)
    <<~HTML.html_safe
      <div class="rounded-xl border border-gray-200 bg-card p-4 shadow-sm">
        <div class="text-sm font-semibold text-gray-900">#{title}</div>
        <div class="mt-1 text-xs text-gray-500">#{subtitle}</div>
      </div>
    HTML
  end
end
