# frozen_string_literal: true

# Bottom-center status pill. Rendered here with position: :none so each variant
# shows inline; in the app it's :fixed (the live sync indicator) or :absolute
# (the Skim undo toast).
class StatusFeedbackComponentPreview < Lookbook::Preview
  # Ambient sync state: a spinner and a label, the whole pill links to history.
  def syncing
    render(Campbooks::StatusFeedback.new(
      position: :none, spinner: true, message: "Syncing your inbox", href: "#"
    ))
  end

  # An action just taken, with an Undo (the Skim mode shape).
  def with_undo
    render(Campbooks::StatusFeedback.new(
      position: :none, message: "Archived 3 emails", action: { label: "Undo", href: "#" }
    ))
  end

  # A confirmation with a leading icon and no action.
  def confirmation
    render(Campbooks::StatusFeedback.new(
      position: :none, message: "Reply sent",
      icon: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>'
    ))
  end

  # Colored variant badge — the Skim success toast shape, matching the action snackbar.
  def variant_success_with_undo
    render(Campbooks::StatusFeedback.new(
      position: :none, variant: :success, message: "Archived 3 emails",
      action: { label: "Undo", href: "#" }
    ))
  end

  # Error variant (red badge), no action.
  def variant_error
    render(Campbooks::StatusFeedback.new(
      position: :none, variant: :error, message: "Couldn't archive — still in your inbox"
    ))
  end

  # Just a message.
  def message_only
    render(Campbooks::StatusFeedback.new(position: :none, message: "Saved"))
  end
end
