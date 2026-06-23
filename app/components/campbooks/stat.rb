module Campbooks
  class Stat < Campbooks::Base
    VALUE_CLASSES = {
      neutral:   "text-foreground",
      attention: "text-attention-700",
      success:   "text-success",
      danger:    "text-destructive",
      info:      "text-primary"
    }.freeze

    def initialize(value:, label:, variant: :neutral, href: nil, **attrs)
      @value = value
      @label = label
      @variant = variant
      @href = href
      @attrs = attrs
    end

    def with_icon(&block)
      @icon = block
    end

    def view_template
      custom_class = @attrs.delete(:class)
      merged = class_names(
        "bg-card text-card-foreground rounded-lg shadow-sm border border-border px-4 py-3",
        @href ? "block transition-[box-shadow,transform] duration-150 ease-out hover:shadow-md hover:-translate-y-0.5" : nil,
        custom_class
      )

      content = render_stat_content

      if @href
        a(href: @href, class: "block", **@attrs) do
          article(class: merged) { raw(safe(content)) }
        end
      else
        article(class: merged, **@attrs) { raw(safe(content)) }
      end
    end

    private

    def render_stat_content
      capture do
        div(class: "flex items-center justify-between") do
          div do
            div(class: class_names("text-xl font-bold tabular-nums tracking-tight", VALUE_CLASSES[@variant])) { @value.to_s }
            p(class: "mt-0.5 text-xs text-muted-foreground") { @label }
          end
          if @icon
            div(class: "flex-shrink-0 ml-3", &@icon)
          end
        end
      end
    end
  end
end
