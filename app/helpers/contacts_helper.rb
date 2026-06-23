module ContactsHelper
  def relationship_badge(relationship_type)
    return nil unless relationship_type

    colors = {
      "client" => "bg-green-100 text-green-700",
      "vendor" => "bg-blue-100 text-blue-700",
      "partner" => "bg-purple-100 text-purple-700",
      "service_provider" => "bg-orange-100 text-orange-700",
      "colleague" => "bg-gray-100 text-gray-700",
      "personal" => "bg-pink-100 text-pink-700",
      "unknown" => "bg-gray-100 text-gray-500"
    }
    css = colors[relationship_type] || "bg-gray-100 text-gray-500"

    content_tag(:span, t("helpers.relationship.#{relationship_type}", default: relationship_type.humanize),
      class: "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium #{css}")
  end

  STATE_TOAST_KEYS = {
    "star" => "starred", "unstar" => "unstarred", "allow" => "allowed",
    "block" => "blocked", "unblock" => "unblocked"
  }.freeze

  # Toast copy for a contact list-state change (see contacts/set_state).
  def contact_state_toast(contact, state)
    key = STATE_TOAST_KEYS[state.to_s] || "updated"
    t("contacts.state.#{key}_toast", name: contact.display_name)
  end

  # Small inline icons for the star/block buttons on rows and the profile header.
  def contact_star_icon(filled: false)
    raw(%(<svg class="w-4 h-4" viewBox="0 0 24 24" fill="#{filled ? 'currentColor' : 'none'}" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>))
  end

  def contact_block_icon
    raw(%(<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/></svg>))
  end

  def contact_unblock_icon
    raw(%(<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 12l2 2 4-4"/><circle cx="12" cy="12" r="9"/></svg>))
  end
end
