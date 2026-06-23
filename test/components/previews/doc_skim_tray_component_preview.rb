# frozen_string_literal: true

class DocSkimTrayComponentPreview < ViewComponent::Preview
  RINGS = [
    { category: "accounting", label: "Accounting", count: 6 },
    { category: "insurance", label: "Insurance", count: 2 },
    { category: "vehicles", label: "Vehicles", count: 1 },
    { category: "other", label: "Other", count: 3 }
  ].freeze

  # @label Tray (Review all + category rings)
  def tray
    render Campbooks::DocSkimTray.new(rings: RINGS)
  end

  # @label Ring — category
  def ring
    render Campbooks::DocSkimRing.new(category: "accounting", label: "Accounting", count: 6)
  end

  # @label Ring — "Review all" lead
  def all_ring
    render Campbooks::DocSkimRing.new(category: nil, label: "Review all", count: 12)
  end

  # @label Ring — large count (abbreviated to 1.2k)
  def large_count
    render Campbooks::DocSkimRing.new(category: "correspondence", label: "Correspondence", count: 1240)
  end

  # @label Tray — every category
  def all_categories
    rings = DocumentType::CATEGORIES.each_with_index.map do |category, i|
      { category: category, label: category.humanize, count: (i + 1) * 2 }
    end
    render Campbooks::DocSkimTray.new(rings: rings)
  end
end
