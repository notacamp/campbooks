# frozen_string_literal: true

class CategoryChipComponentPreview < ViewComponent::Preview
  # @label All categories (md)
  def all_categories
    render Campbooks::CategoryChipVariants.new(size: :md)
  end

  # @label Compact (sm)
  def compact
    render Campbooks::CategoryChipVariants.new(size: :sm)
  end

  # @label Icon only
  def icon_only
    render Campbooks::CategoryChipVariants.new(size: :md, label: false)
  end

  def personal
    render Campbooks::CategoryChip.new(category: :personal)
  end

  def important
    render Campbooks::CategoryChip.new(category: :important)
  end

  def notifications
    render Campbooks::CategoryChip.new(category: :notifications)
  end

  # @label Skimmable inbox (noise recedes, what needs you surfaces)
  def skim_inbox
    render Campbooks::SkimRowsDemo.new
  end
end
