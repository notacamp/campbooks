# frozen_string_literal: true

# Preview for the compact star + block/unblock controls embedded in the contact
# hover card (Campbooks::ContactPopover). Each posts to contacts#set_state and is
# re-rendered in place by set_state.turbo_stream after a change. Uses unsaved
# Contact records with a stub id so the forms have a valid action. Top-level class
# to match the file path (Zeitwerk).
class ContactStateActionsComponentPreview < ViewComponent::Preview
  # @label Neutral (Star / Block)
  def neutral
    render Campbooks::ContactStateActions.new(contact: contact_with)
  end

  # @label Starred (Starred / Block)
  def starred
    render Campbooks::ContactStateActions.new(contact: contact_with(starred_at: Time.current))
  end

  # @label Blocked (Star / Unblock)
  def blocked
    render Campbooks::ContactStateActions.new(contact: contact_with(list_status: :blocked))
  end

  private

  def contact_with(**attrs)
    Contact.new({ id: 1, workspace_id: 0, email: "sender@example.com" }.merge(attrs))
  end
end
