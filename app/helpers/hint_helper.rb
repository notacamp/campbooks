# frozen_string_literal: true

# Helpers for wiring keyboard-shortcut hint tooltips onto action controls.
# Used in Phlex components (via include in Campbooks::Base) and ERB views.
#
# Phlex usage:
#   button(data: { people_reply: true, **hint_data("Reply", key: "r") },
#          aria: { label: "Reply", **hint_aria("r") })
#
# ERB usage:
#   tag.button(**hint_html_attrs("Archive", key: "e"), aria_label: "Archive")
module HintHelper
  # Returns the data-* attributes for a hinted control (Phlex style, nested hash).
  # Merges into an existing data: {} hash with the ** splat operator.
  #   hint_data("Reply", key: "r")
  #   # => { hint: "Reply", hint_key: "r" }
  #   hint_data("People", key: "g p", placement: :right)
  #   # => { hint: "People", hint_key: "g p", hint_placement: "right" }
  def hint_data(label, key: nil, placement: nil)
    { hint: label, hint_key: key, hint_placement: placement }.compact
  end

  # Returns the aria-* sub-hash for a hinted control (Phlex style).
  # When a key string is given, produces { keyshortcuts: "<aria notation>" }.
  # When the key is nil/blank, returns {}.
  #   aria: { label: "Reply", **hint_aria("r") }
  #   # => { label: "Reply", keyshortcuts: "r" }
  def hint_aria(key)
    ks = aria_keyshortcuts_for(key)
    ks ? { keyshortcuts: ks } : {}
  end

  # Returns a flat hash of HTML attribute strings suitable for ERB tag helpers.
  #   tag.button(**hint_html_attrs("Archive", key: "e"), aria_label: "Archive")
  def hint_html_attrs(label, key: nil, placement: nil)
    attrs = { "data-hint" => label }
    attrs["data-hint-key"] = key if key.present?
    attrs["data-hint-placement"] = placement.to_s if placement.present?
    ks = aria_keyshortcuts_for(key)
    attrs["aria-keyshortcuts"] = ks if ks
    attrs
  end

  # Converts a display-notation key string to the WAI-ARIA keyshortcuts format.
  # Modifier symbols are mapped to their ARIA equivalents; sequences ("g p") are
  # kept verbatim (same convention as Campbooks::NavRail).
  #
  #   aria_keyshortcuts_for("⌘K")   # => "Meta+K"
  #   aria_keyshortcuts_for("⇧I")   # => "Shift+I"
  #   aria_keyshortcuts_for("⌥X")   # => "Alt+X"
  #   aria_keyshortcuts_for("⌃X")   # => "Control+X"
  #   aria_keyshortcuts_for("⏎")    # => "Enter"
  #   aria_keyshortcuts_for("Esc")  # => "Escape"
  #   aria_keyshortcuts_for("→")    # => "ArrowRight"
  #   aria_keyshortcuts_for("←")    # => "ArrowLeft"
  #   aria_keyshortcuts_for("↑")    # => "ArrowUp"
  #   aria_keyshortcuts_for("↓")    # => "ArrowDown"
  #   aria_keyshortcuts_for("g p")  # => "g p"   (sequence: kept verbatim)
  #   aria_keyshortcuts_for("e")    # => "e"
  #   aria_keyshortcuts_for(nil)    # => nil
  def aria_keyshortcuts_for(key)
    return nil if key.blank?

    case key
    when /\A⌘(.+)\z/ then "Meta+#{$1}"
    when /\A⇧(.+)\z/ then "Shift+#{$1}"
    when /\A⌥(.+)\z/ then "Alt+#{$1}"
    when /\A⌃(.+)\z/ then "Control+#{$1}"
    when "⏎"          then "Enter"
    when "Esc"             then "Escape"
    when "→"          then "ArrowRight"
    when "←"          then "ArrowLeft"
    when "↑"          then "ArrowUp"
    when "↓"          then "ArrowDown"
    else key
    end
  end
end
