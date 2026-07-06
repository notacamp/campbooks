# frozen_string_literal: true

module Campbooks
  class PageHeader < Campbooks::Base
    # @param title [String] required heading text
    # @param subtitle [String, nil] optional smaller text below title
    # @param spacing [Symbol] :md (mb-6), :lg (mb-8)
    # @param back_href [String, nil] optional "back" link target rendered above the title
    # @param back_label [String, nil] optional label for the back link (arrow shows regardless)
    def initialize(title:, subtitle: nil, spacing: :md, back_href: nil, back_label: nil, **attrs)
      @title = title
      @subtitle = subtitle
      @spacing = spacing
      @back_href = back_href
      @back_label = back_label
      @attrs = attrs
    end

    def with_actions(&block)
      @actions = block
    end

    def view_template(&content)
      # Execute the content block first so slot methods (e.g. `with_actions`)
      # register before we render. Capture any *direct* output so block-as-actions
      # usage (no `with_actions` call) still lands in the actions slot.
      captured = content ? capture(&content) : ""

      div(class: class_names("flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between", SPACING_CLASSES[@spacing]), **@attrs) do
        div(class: "min-w-0") do
          if @back_href
            a(href: @back_href, class: "inline-flex items-center gap-1 mb-1.5 -ml-0.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground") do
              raw safe(BACK_ARROW_SVG)
              span { @back_label } if @back_label
            end
          end
          h1(class: "text-xl sm:text-2xl font-bold tracking-tight text-foreground") { @title }
          if @subtitle
            p(class: "mt-1 text-sm text-muted-foreground") { @subtitle }
          end
        end

        if @actions
          div(class: "flex items-center flex-wrap gap-3") { __yield_content__(&@actions) }
        elsif captured.present?
          div(class: "flex items-center flex-wrap gap-3") { raw(safe(captured)) }
        end
      end
    end

    private

    # Heroicons "arrow-left" (24×24 outline). Inlined like Campbooks::Icon since
    # the curated Icon set carries only folder glyphs, no navigation arrows.
    BACK_ARROW_SVG =
      '<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" ' \
      'stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' \
      '<path d="M10.5 19.5 3 12m0 0 7.5-7.5M3 12h18"/></svg>'

    SPACING_CLASSES = {
      md: "mb-6",
      lg: "mb-8"
    }.freeze
  end
end
