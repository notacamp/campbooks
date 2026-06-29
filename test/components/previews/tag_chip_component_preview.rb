# frozen_string_literal: true

class TagChipComponentPreview < ViewComponent::Preview
  # Lightweight stand-in so the preview needs no DB records.
  Tag = Struct.new(:name, :color)

  # @!group Sizes
  # Detail-pane size.
  def medium
    render(Campbooks::TagChip.new(tag: Tag.new("Invoices", "#2563eb"), size: :md))
  end

  # Compact size used in inbox thread rows and search results.
  def small
    render(Campbooks::TagChip.new(tag: Tag.new("Invoices", "#2563eb"), size: :sm))
  end
  # @!endgroup

  # Removable variant (the × button used in the detail-pane picker).
  def removable
    render(Campbooks::TagChip.new(
      tag: Tag.new("Clients", "#16a34a"), size: :md, removable: true,
      remove_data: { "action" => "email-tags#remove" }
    ))
  end

  # Long names truncate rather than overflow.
  def long_name
    render(Campbooks::TagChip.new(tag: Tag.new("A very long tag name that should truncate", "#9333ea"), size: :md))
  end

  # Several chips wrapping — checks spacing at narrow (mobile) widths.
  def group
    render_with_template(locals: {
      tags: [
        Tag.new("Invoices", "#2563eb"), Tag.new("Clients", "#16a34a"),
        Tag.new("Travel", "#9333ea"), Tag.new("Tax 2025", "#ea580c")
      ]
    })
  end
end
