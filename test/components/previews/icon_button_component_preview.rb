# frozen_string_literal: true

class IconButtonComponentPreview < ViewComponent::Preview
  # Default icon button (md size).
  def default
    render(Campbooks::IconButton.new(aria_label: "Settings") { settings_svg })
  end

  # Small icon button.
  def sm
    render(Campbooks::IconButton.new(size: :sm, aria_label: "Settings") { settings_svg })
  end

  # Medium icon button.
  def md
    render(Campbooks::IconButton.new(size: :md, aria_label: "Settings") { settings_svg })
  end

  # Large icon button.
  def lg
    render(Campbooks::IconButton.new(size: :lg, aria_label: "Settings") { settings_svg })
  end

  # All sizes side by side.
  def sizes
    html = [ render(Campbooks::IconButton.new(size: :sm, aria_label: "Settings") { settings_svg }),
            render(Campbooks::IconButton.new(size: :md, aria_label: "Settings") { settings_svg }),
            render(Campbooks::IconButton.new(size: :lg, aria_label: "Settings") { settings_svg }) ].join
    "<div class=\"flex items-center gap-2 p-6\">#{html}</div>".html_safe
  end

  # Icon button rendered as an `<a>` tag.
  def as_link
    render(Campbooks::IconButton.new(href: "#", aria_label: "Home") { home_svg })
  end

  private

  def settings_svg
    '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">' \
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>' \
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>' \
      "</svg>".html_safe
  end

  def home_svg
    '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" aria-hidden="true">' \
      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"/>' \
      "</svg>".html_safe
  end
end
