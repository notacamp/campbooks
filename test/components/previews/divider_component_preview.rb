# frozen_string_literal: true

class DividerComponentPreview < ViewComponent::Preview
  # Simple horizontal rule without a label.
  def default
    render(Campbooks::Divider.new)
  end

  # Divider with centered text label.
  def with_label
    render(Campbooks::Divider.new(label: "or"))
  end

  # Divider with a custom label.
  def custom_label
    render(Campbooks::Divider.new(label: "continue with"))
  end

  # Divider used between sections of content.
  def between_content
    content_tag(:div, class: "p-6 space-y-4") do
      tag.p("Above the divider", class: "text-sm text-gray-600") +
        render(Campbooks::Divider.new(label: "or")) +
        tag.p("Below the divider", class: "text-sm text-gray-600")
    end
  end
end
