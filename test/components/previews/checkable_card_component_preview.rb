# frozen_string_literal: true

class CheckableCardComponentPreview < ViewComponent::Preview
  # Horizontal variant with color dot and description (unchecked)
  def horizontal
    render Campbooks::CheckableCard.new(
      name: "doc_type[]",
      value: "invoice",
      title: "Invoice",
      description: "Vendor invoices, bills, and payment requests with line items",
      color: "#ef4444",
      variant: :horizontal
    )
  end

  # Horizontal variant checked
  def horizontal_checked
    render Campbooks::CheckableCard.new(
      name: "doc_type[]",
      value: "invoice",
      title: "Invoice",
      description: "Vendor invoices, bills, and payment requests with line items",
      color: "#ef4444",
      variant: :horizontal,
      checked: true
    )
  end

  # Horizontal without color dot
  def horizontal_no_color
    render Campbooks::CheckableCard.new(
      name: "doc_type[]",
      value: "receipt",
      title: "Receipt",
      description: "Store and transaction receipts",
      variant: :horizontal
    )
  end

  # Horizontal without description
  def horizontal_no_description
    render Campbooks::CheckableCard.new(
      name: "doc_type[]",
      value: "contract",
      title: "Contract",
      color: "#3b82f6",
      variant: :horizontal
    )
  end

  # Compact variant with color dot (unchecked)
  def compact
    render Campbooks::CheckableCard.new(
      name: "tags[]",
      value: "newsletter",
      title: "Newsletter",
      color: "#8b5cf6",
      variant: :compact
    )
  end

  # Compact variant checked
  def compact_checked
    render Campbooks::CheckableCard.new(
      name: "tags[]",
      value: "newsletter",
      title: "Newsletter",
      color: "#8b5cf6",
      variant: :compact,
      checked: true
    )
  end

  # Compact without color dot
  def compact_no_color
    render Campbooks::CheckableCard.new(
      name: "tags[]",
      value: "update",
      title: "Update",
      variant: :compact
    )
  end

  # Radio type (horizontal, unchecked)
  def radio_type
    render Campbooks::CheckableCard.new(
      name: "selection",
      value: "option_a",
      type: :radio,
      title: "Option A",
      description: "Radio-style selection card with exclusive choice",
      color: "#10b981",
      variant: :horizontal
    )
  end

  # Horizontal variants in a grid (shows checked/unchecked, with/without color, with/without description)
  def horizontal_gallery
    html = [
      render(Campbooks::CheckableCard.new(name: "types[]", value: "invoice", title: "Invoice", description: "Payment requests with line items", color: "#ef4444", variant: :horizontal)),
      render(Campbooks::CheckableCard.new(name: "types[]", value: "receipt", title: "Receipt", description: "Store and transaction receipts", color: "#f59e0b", variant: :horizontal)),
      render(Campbooks::CheckableCard.new(name: "types[]", value: "contract", title: "Contract", description: "Signed agreements and legal docs", color: "#3b82f6", variant: :horizontal, checked: true)),
      render(Campbooks::CheckableCard.new(name: "types[]", value: "report", title: "Report", description: "Monthly and quarterly summaries", color: "#8b5cf6", variant: :horizontal))
    ].join
    "<div class=\"grid grid-cols-1 sm:grid-cols-2 gap-2 p-6\">#{html}</div>".html_safe
  end

  # Compact variants in a grid (shows checked/unchecked, with/without color)
  def compact_gallery
    html = [
      render(Campbooks::CheckableCard.new(name: "tags[]", value: "newsletter", title: "Newsletter", color: "#8b5cf6", variant: :compact)),
      render(Campbooks::CheckableCard.new(name: "tags[]", value: "alert", title: "Alert", color: "#ef4444", variant: :compact, checked: true)),
      render(Campbooks::CheckableCard.new(name: "tags[]", value: "update", title: "Update", color: "#3b82f6", variant: :compact)),
      render(Campbooks::CheckableCard.new(name: "tags[]", value: "promo", title: "Promotion", color: "#f59e0b", variant: :compact))
    ].join
    "<div class=\"grid grid-cols-2 sm:grid-cols-4 gap-2 p-6\">#{html}</div>".html_safe
  end
end
