module Campbooks
  class Card < Campbooks::Base
    PADDING = {
      none: "",
      xs: "p-4",
      sm: "p-3",
      md: "p-5",
      lg: "p-6"
    }.freeze

    def initialize(padding: :md, hover: false, overflow: :visible, **attrs)
      @padding = padding
      @hover = hover
      @overflow = overflow
      @attrs = attrs
      @header_divider = false
    end

    def with_header(divider: false, &block)
      @header = block
      @header_divider = divider
    end

    def with_body(&block)
      @body = block
    end

    def with_footer(&block)
      @footer = block
    end

    def view_template(&content)
      captured = content ? capture(&content) : ""

      custom_class = @attrs.delete(:class)
      merged = class_names(card_classes, custom_class)

      article(class: merged, **@attrs) do
        if slots_used?
          render_header
          if @body
            div(class: "px-6 py-4", &@body)
          elsif captured.present?
            div(class: "px-6 py-4") { raw(safe(captured)) }
          end
          render_footer
        else
          raw(safe(captured)) if captured.present?
        end
      end
    end

    private

    def render_header
      return unless @header

      div(class: class_names("px-6 py-4", ("border-b border-border" if @header_divider)),
        &@header)
    end

    def render_footer
      return unless @footer

      div(class: "px-6 py-3 border-t border-border text-right", &@footer)
    end

    def card_classes
      tokens = [ "bg-card text-card-foreground rounded-xl shadow-sm border border-border" ]
      tokens << PADDING[@padding] unless slots_used?
      tokens << "hover:shadow-md transition-shadow" if @hover
      tokens << "overflow-hidden" if @overflow == :hidden
      class_names(tokens)
    end

    def slots_used?
      @header || @body || @footer
    end
  end
end
