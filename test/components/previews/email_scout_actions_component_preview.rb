# frozen_string_literal: true

class EmailScoutActionsComponentPreview < ViewComponent::Preview
  # Drawer surface: Scout's read plus the full chip set, reply included (posts
  # draft_reply, whose editable preview lands in the drawer's compose slot).
  def drawer
    render(Campbooks::EmailScoutActions.new(message: sample_message, surface: :drawer, can_send: true))
  end

  # Full detail pane: same strip, reply posts with surface=detail.
  def detail
    render(Campbooks::EmailScoutActions.new(message: sample_message, surface: :detail, can_send: true))
  end

  # Read-only shared inbox: the read and non-send actions show, but no reply.
  def read_only
    render(Campbooks::EmailScoutActions.new(message: sample_message, surface: :drawer, can_send: false))
  end

  # Just the suggestions, no Scout read line.
  def without_note
    render(Campbooks::EmailScoutActions.new(message: sample_message, surface: :drawer, can_send: true, show_note: false))
  end

  private

  def sample_message
    account = EmailAccount.new(id: 1, color: "#6366f1", email_address: "me@example.com")
    message = EmailMessage.new(
      id: 1,
      from_address: "emma@maplelodge.com",
      subject: "Invoice #2025-114 needs your sign-off",
      ai_action_prompt: "Emma's asking you to approve invoice #2025-114 by Friday — I can draft a reply.",
      ai_suggested_actions: [
        { "tool" => "add_tag", "args" => { "tag_name" => "invoices" } },
        { "tool" => "archive" },
        { "tool" => "create_calendar_event", "args" => { "title" => "Sign off invoice #2025-114" } }
      ]
    )
    message.email_account = account
    message
  end
end
