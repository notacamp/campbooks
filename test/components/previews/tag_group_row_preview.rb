# frozen_string_literal: true

# Previews for the collapsed inbox tag-group row. Senders are plain hashes
# (no DB records needed); the wrapper div mimics the inbox list column.
class TagGroupRowPreview < ViewComponent::Preview
  # All four built-in default groups as they appear injected into the inbox list.
  def default_groups
    render_with_template
  end

  def single_sender
    render Campbooks::TagGroupRow.new(
      label: "Notifications", count: 1, color: "#767988", href: "#",
      senders: [ { email: "builds@ci.example" } ]
    )
  end

  # More senders than the facepile shows — the extras fold into a "+N" chip.
  def many_senders
    render Campbooks::TagGroupRow.new(
      label: "Newsletters & promos", count: 12, color: "#d44996", href: "#",
      senders: [
        { email: "deals@shop.example" }, { email: "news@daily.example" },
        { email: "offers@store.example" }, { email: "digest@blog.example" }
      ]
    )
  end

  # A group with no recent senders falls back to a neutral tag glyph.
  def empty_group
    render Campbooks::TagGroupRow.new(label: "Updates", count: 2, color: "#00a8a8", href: "#")
  end

  # Long custom group names truncate instead of wrapping.
  def long_label
    render Campbooks::TagGroupRow.new(
      label: "Quarterly supplier invoices awaiting reconciliation", count: 4,
      color: "#595dec", href: "#",
      senders: [ { email: "billing@vendor.example" }, { email: "ap@supplier.example" } ]
    )
  end

  # A custom group whose tags have no color — the identity dot is simply omitted.
  def no_color
    render Campbooks::TagGroupRow.new(
      label: "Receipts", count: 3, href: "#",
      senders: [ { email: "store@shop.example" } ]
    )
  end

  # Select-mode active — the checkbox is permanently visible (no hover needed).
  # Wrap in a group/select div with data-select-mode="on" to simulate the
  # email-selection controller root that drives the Tailwind modifier.
  def select_mode
    render_with_template
  end
end
