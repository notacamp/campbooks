# frozen_string_literal: true

# Preview for the contact-skim card — the people analogue of Campbooks::SkimCard's
# "pending sender" frame. Uses unsaved Contact records so no DB rows are needed.
# The card sizes itself, so each example renders it directly (matching
# SkimCardComponentPreview). Top-level class to match the file path (Zeitwerk).
class ContactSkimCardComponentPreview < ViewComponent::Preview
  # @label Story frame: with Scout summary
  def analyzed
    render Campbooks::ContactSkimCard.new(contact: contact_with(
      name: "Jamie Sutton",
      email: "jamie.sutton@example.com",
      relationship_type: "client",
      email_count: 72,
      last_email_at: 3.days.ago,
      context_summary: "Primary contact at Acme. Sends monthly invoices and the occasional contract amendment, usually expects a same-day reply."
    ), fill: true, class: "mx-auto max-w-md")
  end

  # @label Story frame: unprofiled sender
  def minimal
    render Campbooks::ContactSkimCard.new(contact: contact_with(
      email: "newsletter@updates.example.com",
      email_count: 2,
      last_email_at: 6.hours.ago
    ), fill: true, class: "mx-auto max-w-md")
  end

  # @label Compact (with progress dots)
  def compact
    render Campbooks::ContactSkimCard.new(contact: contact_with(
      name: "Devon Pereira",
      email: "devon@studio.example.com",
      relationship_type: "vendor",
      email_count: 9,
      last_email_at: 2.days.ago,
      context_summary: "Freelance designer. Occasional project updates and the odd quote."
    ), fill: false, position: 2, total: 5, show_progress: true)
  end

  private

  def contact_with(**attrs)
    Contact.new({ workspace_id: 0 }.merge(attrs))
  end
end
