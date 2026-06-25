# frozen_string_literal: true

class IconComponentPreview < ViewComponent::Preview
  # The default folder glyph.
  def default
    render(Campbooks::Icon.new("folder"))
  end

  # Every icon in the set, labelled — a quick visual check that each path renders.
  def all
    tiles = Campbooks::Icon::NAMES.map do |name|
      "<div class=\"flex flex-col items-center gap-1 w-16\">" \
        "#{render(Campbooks::Icon.new(name, css_class: 'w-6 h-6'))}" \
        "<span class=\"text-[10px] text-muted-foreground\">#{name}</span>" \
        "</div>"
    end.join
    "<div class=\"flex flex-wrap gap-3 p-6 text-foreground\">#{tiles}</div>".html_safe
  end

  # The same glyph at several sizes (driven by css_class).
  def sizes
    html = [ "w-4 h-4", "w-5 h-5", "w-6 h-6", "w-8 h-8" ]
      .map { |c| render(Campbooks::Icon.new("star", css_class: c)) }.join
    "<div class=\"flex items-end gap-3 p-6 text-foreground\">#{html}</div>".html_safe
  end
end
