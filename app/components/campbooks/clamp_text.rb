# frozen_string_literal: true

module Campbooks
  # Long text that clamps to `lines` lines with a "Read more" / "Show less"
  # toggle. The clamp is applied inline (display:-webkit-box + -webkit-line-clamp)
  # rather than via a Tailwind arbitrary `line-clamp-[N]` class, so the
  # clamp_controller can lift it by editing the same inline style — nothing to keep
  # generated or in sync, and no build-time dependency on a dynamic utility.
  #
  # Progressive enhancement: the toggle stays hidden until the controller measures
  # real overflow, so short text shows no button and a no-JS page stays clamped
  # but readable. The passed `class:` styles the wrapper (and the text inherits its
  # typography), so each call site keeps its own treatment — a muted email excerpt
  # or Scout's inline read. Pass the text itself as the block.
  class ClampText < Campbooks::Base
    def initialize(lines: 10, **attrs)
      @lines = lines
      @attrs = attrs
    end

    def view_template(&block)
      content_id = "clamp-#{object_id}"

      div(class: class_names("flex flex-col", @attrs.delete(:class)), **@attrs,
          data: { controller: "clamp", clamp_lines_value: @lines }) do
        p(
          id: content_id,
          style: "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:#{@lines};overflow:hidden",
          data: { clamp_target: "content" }
        ) { yield }

        button(
          type: "button",
          class: "mt-1 hidden self-start text-[12.5px] font-semibold text-muted-foreground underline-offset-2 transition-colors hover:text-foreground hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm",
          aria: { expanded: "false", controls: content_id },
          data: { clamp_target: "button", action: "clamp#toggle" }
        ) do
          span(data: { clamp_target: "more" }) { t(".more") }
          span(class: "hidden", data: { clamp_target: "less" }) { t(".less") }
        end
      end
    end
  end
end
