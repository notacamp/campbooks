# frozen_string_literal: true

module Campbooks
  # Brand mark + wordmark.
  #
  # The mark is an Ember gradient tile holding a white "aperture C" glyph — the
  # brand initial drawn as a focus ring with a leading dot, a nod to the
  # product's job of surfacing what needs your attention. The gradient is fixed
  # so the mark reads identically in light and dark; the wordmark uses the
  # theme foreground so it adapts.
  #
  # @param size [Symbol] :sm, :md, :lg
  # @param variant [Symbol] :full (mark + wordmark) or :mark (mark only)
  # @param beta [Boolean] stack a small "Beta" sub-label under the lockup. Gated by the
  #   caller (pass `beta: !self_hosted?`) so self-hosted builds stay unlabelled.
  class Logo < Campbooks::Base
    SIZES = {
      sm: { tile: "size-6 rounded-[7px]", glyph: "size-4",      word: "text-base" },
      md: { tile: "size-7 rounded-lg",    glyph: "size-[18px]", word: "text-lg" },
      lg: { tile: "size-9 rounded-xl",    glyph: "size-6",      word: "text-2xl" }
    }.freeze

    TILE_STYLE = "background-image: var(--ember); box-shadow: var(--ember-glow);"

    def initialize(size: :md, variant: :full, beta: false, **attrs)
      @size = size
      @variant = variant
      @beta = beta
      @attrs = attrs
    end

    def view_template
      cfg = SIZES.fetch(@size)
      custom = @attrs.delete(:class)

      if @variant == :full
        # Mark beside a wordmark column. The Beta tag stacks UNDER "Campbooks" as
        # a sub-label, left-aligned to the wordmark — not trailing it to the right.
        div(class: class_names("inline-flex items-center gap-2.5", custom), **@attrs) do
          mark(cfg)
          span(class: "inline-flex flex-col items-start gap-0.5") do
            span(class: class_names("font-semibold leading-none tracking-tight text-foreground", cfg[:word])) { plain "Campbooks" }
            beta_tag if @beta
          end
        end
      else
        # Mark-only lockup (e.g. the nav rail). The Beta tag stacks UNDER the mark,
        # centered, so it never spills sideways in a narrow rail.
        div(class: class_names("inline-flex flex-col items-center gap-1", custom), **@attrs) do
          mark(cfg)
          beta_tag if @beta
        end
      end
    end

    private

    # The Ember-gradient brand tile holding the aperture-C glyph.
    def mark(cfg)
      span(
        class: class_names(
          cfg[:tile],
          "relative inline-flex items-center justify-center shrink-0",
          "ring-1 ring-inset ring-white/20"
        ),
        style: TILE_STYLE
      ) do
        raw(safe(glyph_svg(cfg[:glyph])))
      end
    end

    # Small accent "Beta" tag, mirroring the standalone beta Badge but sized to
    # lock onto the brand mark/wordmark. Alignment comes from the parent flex-col
    # (left under the wordmark, centered under the mark); `uppercase` renders the
    # stored "Beta" as BETA.
    def beta_tag
      span(
        class: class_names(
          "shrink-0 rounded px-1 py-0.5",
          "text-[10px] font-bold uppercase leading-none tracking-wider",
          "bg-accent-100 text-accent-700"
        )
      ) { t(".beta") }
    end

    # "Layered C": a bold brand-initial C with a lighter inner arc, giving depth
    # that nods to stacked documents without becoming busy.
    def glyph_svg(size_class)
      <<~SVG
        <svg class="#{size_class} text-white" viewBox="0 0 28 28" fill="none" aria-hidden="true">
          <path d="M20.65 17.46 A7.5 7.5 0 1 1 20.65 10.54" stroke="currentColor" stroke-width="2.7" stroke-linecap="round"/>
          <path d="M18.1 16.1 A4.6 4.6 0 1 1 18.1 11.9" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" opacity="0.55"/>
        </svg>
      SVG
    end
  end
end
