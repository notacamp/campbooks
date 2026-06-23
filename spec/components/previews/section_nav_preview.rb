# frozen_string_literal: true

class SectionNavPreview < Lookbook::Preview
  # Mail area: Inbox active.
  def mail
    render(Campbooks::SectionNav.new(
      current: :inbox,
      items: [
        { label: "Inbox", href: "#", key: :inbox },
        { label: "Contacts", href: "#", key: :contacts }
      ]
    ))
  end

  # Scout AI area: Tags active.
  def scout
    render(Campbooks::SectionNav.new(
      current: :tags,
      items: [
        { label: "Chat", href: "#", key: :chat },
        { label: "Tags", href: "#", key: :tags }
      ]
    ))
  end

  # Docs area: Documents active, with a count on Document Types.
  def docs
    render(Campbooks::SectionNav.new(
      current: :documents,
      items: [
        { label: "Documents", href: "#", key: :documents },
        { label: "Document Types", href: "#", key: :document_types, count: 5 }
      ]
    ))
  end

  # Second item active (verifies the indicator tracks the current key).
  def second_active
    render(Campbooks::SectionNav.new(
      current: :document_types,
      items: [
        { label: "Documents", href: "#", key: :documents },
        { label: "Document Types", href: "#", key: :document_types, count: 5 }
      ]
    ))
  end

  # More than two sections (the bar scales without changing).
  def three_sections
    render(Campbooks::SectionNav.new(
      current: :inbox,
      items: [
        { label: "Inbox", href: "#", key: :inbox, count: 12 },
        { label: "Contacts", href: "#", key: :contacts },
        { label: "Snoozed", href: "#", key: :snoozed }
      ]
    ))
  end
end
