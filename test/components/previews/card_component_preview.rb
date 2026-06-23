class CardComponentPreview < ViewComponent::Preview
  def default
    render Campbooks::Card.new { "Basic card with default (md) padding." }
  end

  def padding_sm
    render Campbooks::Card.new(padding: :sm) { "Card with small (p-3) padding." }
  end

  def padding_lg
    render Campbooks::Card.new(padding: :lg) { "Card with large (p-6) padding." }
  end

  def padding_none
    render Campbooks::Card.new(padding: :none) { "Card with no padding." }
  end

  def with_hover
    render Campbooks::Card.new(hover: true) { "Hover over me to see the shadow transition." }
  end

  def with_overflow_hidden
    render Campbooks::Card.new(overflow: :hidden) { "Overflow is hidden on this card." }
  end

  def with_header
    render Campbooks::Card.new do |card|
      card.with_header(divider: true) { "Card Title" }
      card.with_body { "Main body content goes here." }
    end
  end

  def with_header_only
    render Campbooks::Card.new do |card|
      card.with_header { "Just a Header" }
    end
  end

  def with_header_footer
    render Campbooks::Card.new do |card|
      card.with_header(divider: true) { "Card Title" }
      card.with_body { "Your main content lives here. Slots keep the layout consistent." }
      card.with_footer { "<span class=\"text-xs text-gray-500\">Footer note</span>".html_safe }
    end
  end

  def simple_block
    render Campbooks::Card.new(hover: true, padding: :none) do
      "<div class=\"p-6\">
        <h3 class=\"text-base font-semibold text-gray-900 mb-2\">Custom Layout</h3>
        <p class=\"text-sm text-gray-600\">You can also ignore slots and build the entire card content as a block. Great for unique layouts.</p>
      </div>".html_safe
    end
  end
end
