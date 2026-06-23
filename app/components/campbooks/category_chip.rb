# frozen_string_literal: true

module Campbooks
  # Triage category chip. Visual weight encodes how much an email matters: the
  # four "noise" categories share a quiet muted treatment, while the two that need
  # a human — personal (accent) and important (amber) — carry colour. Meaning is
  # conveyed by icon + label, never colour alone (WCAG 2.1 AA).
  class CategoryChip < Campbooks::Base
    CATEGORIES = %i[personal important notifications promotions social updates].freeze


    # Semantic tokens (muted / accent scale) auto-adapt light↔dark. Amber keeps an
    # explicit dark variant since it's a fixed Tailwind hue, not a theme token.
    VARIANT_CLASSES = {
      personal:      "bg-accent-100 text-accent-700",
      important:     "bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300",
      notifications: "bg-muted text-muted-foreground",
      promotions:    "bg-muted text-muted-foreground",
      social:        "bg-muted text-muted-foreground",
      updates:       "bg-muted text-muted-foreground"
    }.freeze

    SIZE_CLASSES = {
      sm: "h-5 gap-1 px-1.5 text-[11px]",
      md: "h-6 gap-1.5 px-2 text-xs"
    }.freeze

    ICON_SIZE = { sm: "w-3 h-3", md: "w-3.5 h-3.5" }.freeze

    BASE = "inline-flex items-center font-medium rounded-md whitespace-nowrap"

    ICONS = {
      personal:      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.121 17.804A13.937 13.937 0 0112 16c2.5 0 4.847.655 6.879 1.804M15 10a3 3 0 11-6 0 3 3 0 016 0z"/>',
      important:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.196-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.783-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z"/>',
      notifications: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"/>',
      promotions:    '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5a1.99 1.99 0 011.414.586l7 7a2 2 0 010 2.828l-5 5a2 2 0 01-2.828 0l-7-7A1.99 1.99 0 013 12V7a4 4 0 014-4z"/>',
      social:        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z"/>',
      updates:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/>'
    }.freeze

    # @param category [Symbol] one of CATEGORIES
    # @param size [Symbol] :sm, :md
    # @param label [Boolean] show the text label (false = icon-only, still labelled for screen readers)
    def initialize(category:, size: :md, label: true, **attrs)
      @category = CATEGORIES.include?(category) ? category : :notifications
      @size = size
      @label = label
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      label_text = category_labels[@category]
      span(
        class: class_names(BASE, SIZE_CLASSES[@size], VARIANT_CLASSES[@category], custom),
        title: label_text,
        **@attrs
      ) do
        svg(
          class: class_names(ICON_SIZE[@size], "flex-shrink-0"),
          fill: "none",
          stroke: "currentColor",
          viewBox: "0 0 24 24",
          aria_hidden: "true"
        ) { raw(safe(ICONS[@category])) }

        if @label
          span { label_text }
        else
          span(class: "sr-only") { label_text }
        end
      end
    end

    private

    def category_labels
      {
        personal:      t(".labels.personal"),
        important:     t(".labels.important"),
        notifications: t(".labels.notifications"),
        promotions:    t(".labels.promotions"),
        social:        t(".labels.social"),
        updates:       t(".labels.updates")
      }
    end
  end
end
