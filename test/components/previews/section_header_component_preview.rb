# frozen_string_literal: true

class SectionHeaderComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::SectionHeader.new) { "Folders" }
  end

  def with_custom_classes
    render(Campbooks::SectionHeader.new(class: "px-3 py-2 bg-gray-50/50 sticky top-0 border-b border-gray-100")) { "Today" }
  end
end
