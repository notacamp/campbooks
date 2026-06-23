module Campbooks
  class EmptyState < Campbooks::Base
    # @param variant [Symbol] :card, :standalone, :inline
    # @param title [String, nil]
    # @param description [String, nil]
    def initialize(variant: :card, title: nil, description: nil, **attrs)
      @variant = variant
      @title = title
      @description = description
      # Pull `class:` out so it merges with each variant's own layout classes
      # (via the Tailwind-aware merger) instead of clobbering them — passing
      # `class: "h-full"` should not strip the variant's `flex`/centering.
      @class = attrs.delete(:class)
      @attrs = attrs
    end

    # Provide an icon as a raw SVG HTML string or as a block.
    # When using a block, it runs in the caller's context (no Phlex helpers).
    def with_icon(svg: nil, &block)
      @icon_content = svg || block
      @icon_is_raw = svg.present?
    end

    # Provide action buttons as raw HTML string or as a block.
    # When using a block, it runs in the caller's context (no Phlex helpers).
    def with_actions(html: nil, &block)
      @actions_content = html || block
      @actions_is_raw = html.present?
    end

    def view_template(&content)
      # Execute block first so slots are set up before we render
      captured = content ? capture(&content) : ""

      case @variant
      when :standalone
        standalone_template(captured)
      when :inline
        inline_template(captured)
      else
        card_template(captured)
      end
    end

    private

    def standalone_template(captured)
      div(class: class_names("flex items-center justify-center py-20", @class), role: "status", **@attrs) do
        div(class: "text-center max-w-md px-6") do
          render_icon(:standalone)
          render_title(:standalone)
          render_description(:standalone)
          render_actions
          raw(safe(captured)) if captured.present?
        end
      end
    end

    def card_template(captured)
      div(class: class_names("px-6 py-12 text-center", @class), role: "status", **@attrs) do
        render_icon(:card)
        render_title(:card)
        render_description(:card)
        render_actions
        raw(safe(captured)) if captured.present?
      end
    end

    def inline_template(captured)
      div(class: class_names("py-4 text-center", @class), role: "status", **@attrs) do
        if @title
          p(class: "text-sm text-muted-foreground") { @title }
        end
        raw(safe(captured)) if captured.present?
      end
    end

    def render_icon(variant)
      return unless @icon_content

      icon_element = if @icon_is_raw
        proc { raw(safe(@icon_content)) }
      else
        @icon_content
      end

      if variant == :standalone
        div(
          class: "w-16 h-16 rounded-full bg-card shadow-sm border border-border mx-auto flex items-center justify-center mb-4",
          &icon_element
        )
      else
        if @icon_is_raw
          raw(safe(@icon_content))
        else
          __yield_content__(&icon_element)
        end
      end
    end

    def render_title(variant)
      return unless @title

      if variant == :standalone
        h2(class: "text-lg font-semibold tracking-tight text-foreground mb-2") { @title }
      else
        h2(class: class_names("text-sm font-medium text-foreground", ("mt-4" if @icon_content))) { @title }
      end
    end

    def render_description(variant)
      return unless @description

      if variant == :standalone
        p(class: "text-sm text-muted-foreground mb-6") { @description }
      else
        p(class: "mt-1 text-sm text-muted-foreground") { @description }
      end
    end

    def render_actions
      return unless @actions_content

      div(class: "mt-4 flex justify-center gap-3") do
        if @actions_is_raw
          raw(safe(@actions_content))
        else
          __yield_content__(&@actions_content)
        end
      end
    end
  end
end
