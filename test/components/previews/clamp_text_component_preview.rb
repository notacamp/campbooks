# frozen_string_literal: true

class ClampTextComponentPreview < ViewComponent::Preview
  SHORT = "Scout read this and there is nothing here that needs you right now."
  LONG = "Thanks for the detailed update on the venue contract. A few things stand " \
         "out: the deposit is due two weeks earlier than we discussed, the cancellation " \
         "window shrank from 30 days to 14, and they have added a separate cleaning fee " \
         "that was not in the original quote. None of these are dealbreakers on their own, " \
         "but together they shift the total by a little over twelve percent, so it is worth " \
         "a quick call before you sign. I would also double-check the insurance clause on " \
         "page four — it asks you to name them as an additional insured, which usually means " \
         "looping in your broker. Everything else looks standard and matches what they sent " \
         "in May, so once those three points are settled you should be clear to proceed."

  # Long text clamped to 10 lines (the home-feed default). The toggle appears once
  # the controller measures real overflow; expanding swaps "Read more" for "Show less".
  def default
    render(Campbooks::ClampText.new(class: "max-w-md text-sm leading-relaxed text-muted-foreground")) { LONG }
  end

  # Short text: no toggle, because nothing is hidden.
  def short
    render(Campbooks::ClampText.new(class: "max-w-md text-sm leading-relaxed text-muted-foreground")) { SHORT }
  end

  # A tighter clamp (3 lines) for denser surfaces.
  def three_lines
    render(Campbooks::ClampText.new(lines: 3, class: "max-w-md text-sm leading-relaxed text-muted-foreground")) { LONG }
  end

  # Mirrors Scout's inline read: a bold label riding inside the clamped text.
  def with_inline_label
    render(Campbooks::ClampText.new(class: "max-w-md text-[13px] leading-relaxed text-foreground/80")) do
      tag.span("Scout", class: "font-semibold text-foreground") + " ".html_safe + LONG
    end
  end
end
