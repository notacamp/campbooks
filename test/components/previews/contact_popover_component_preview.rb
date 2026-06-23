# frozen_string_literal: true

# Preview for the app-wide contact hover card. In the app it is fetched lazily by
# the `contact-popover` Stimulus controller (off any Campbooks::ContactAvatar) and
# injected into the document body; here each state renders directly. Uses unsaved
# Contact records (with a stub id so the set_state forms have a valid action) so no
# DB rows are needed. Top-level class to match the file path (Zeitwerk).
class ContactPopoverComponentPreview < ViewComponent::Preview
  # @label Neutral sender (can star or block)
  def neutral
    render Campbooks::ContactPopover.new(contact: contact_with(
      name: "Jamie Sutton",
      email: "jamie.sutton@example.com",
      relationship_type: "client",
      email_count: 72,
      last_email_at: 3.days.ago,
      context_summary: "Primary contact at Acme. Sends monthly invoices and the occasional contract amendment."
    ))
  end

  # @label Starred sender
  def starred
    render Campbooks::ContactPopover.new(contact: contact_with(
      name: "Devon Pereira",
      email: "devon@studio.example.com",
      relationship_type: "vendor",
      email_count: 18,
      last_email_at: 1.day.ago,
      starred_at: Time.current
    ))
  end

  # @label Blocked sender (can unblock)
  def blocked
    render Campbooks::ContactPopover.new(contact: contact_with(
      email: "promotions@spammy.example.com",
      email_count: 41,
      last_email_at: 5.hours.ago,
      list_status: :blocked
    ))
  end

  # @label Minimal (unprofiled sender)
  def minimal
    render Campbooks::ContactPopover.new(contact: contact_with(
      email: "newsletter@updates.example.com",
      email_count: 2,
      last_email_at: 6.hours.ago
    ))
  end

  private

  def contact_with(**attrs)
    Contact.new({ id: 1, workspace_id: 0 }.merge(attrs))
  end
end
