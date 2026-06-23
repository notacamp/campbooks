# frozen_string_literal: true

module Campbooks
  # Contextual secondary navigation. A thin bar that sits directly under the
  # primary topbar and exposes the sub-sections of the area you're in
  # (e.g. Mail → Inbox / Contacts, Docs → Documents / Document Types).
  #
  # Subordinate to the topbar by construction: the topbar carries filled accent
  # pills, so this bar uses lighter underline tabs (the documented nav indicator
  # in DESIGN.md, never a side stripe). Same chrome (bg-sidebar, border-border)
  # so the two read as one stacked header.
  #
  #   render(Campbooks::SectionNav.new(
  #     current: :documents,
  #     items: [
  #       { label: "Documents", href: documents_path, key: :documents },
  #       { label: "Pending",   href: documents_path(status: "pending"), key: :pending, count: 5 }
  #     ]
  #   ))
  class SectionNav < Campbooks::Base
    # @param items [Array<Hash>] each { label:, href:, key:, count? (Integer), data? (Hash) }
    # @param current [Symbol, String] key of the active item
    # @param label [String] accessible name for the nav landmark
    def initialize(items:, current:, label: nil, **attrs)
      @items = items
      @current = current.to_s
      @label = label
      @attrs = attrs
    end

    def view_template
      nav(
        class: class_names(
          "bg-sidebar/80 backdrop-blur-md border-b border-border px-4 sm:px-6 lg:px-8",
          @attrs.delete(:class)
        ),
        aria_label: @label || t(".default_label"),
        **@attrs
      ) do
        div(class: "flex items-center gap-5 h-10 -mb-px") do
          @items.each { |item| render_item(item) }
        end
      end
    end

    private

    def render_item(item)
      active = item[:key].to_s == @current
      attrs = { href: item[:href], class: item_classes(active) }
      attrs[:aria_current] = "page" if active
      attrs[:data] = item[:data] if item[:data]

      a(**attrs) do
        span { item[:label] }
        if (count = item[:count]) && count.positive?
          span(class: count_classes(active)) { count.to_s }
        end
      end
    end

    def item_classes(active)
      class_names(
        "inline-flex items-center gap-1.5 h-10 px-0.5 border-b-2 text-[13px] transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-sidebar rounded-sm",
        active ? "border-accent-600 text-accent-700 font-medium" : "border-transparent text-gray-500 hover:text-gray-900 hover:border-gray-300"
      )
    end

    def count_classes(active)
      class_names(
        "min-w-[18px] h-4 px-1 text-[10px] font-bold rounded-full inline-flex items-center justify-center",
        active ? "bg-accent-100 text-accent-700" : "bg-gray-100 text-gray-500"
      )
    end
  end
end
