# frozen_string_literal: true

class SkimTrayComponentPreview < ViewComponent::Preview
  # @label Tray (Skim all + theme rings)
  def tray
    render Campbooks::SkimTray.new(rings: Campbooks::SkimCardsDemo::RINGS)
  end

  # @label Tray — inbox + documents ("Skim all" folds in the doc steps)
  # "Skim all" badge = inbox steps (5) + doc steps (3) = 8, and chains the inbox
  # walk into document review.
  def tray_with_documents
    render Campbooks::SkimTray.new(rings: Campbooks::SkimCardsDemo::RINGS, doc_count: 3)
  end

  # @label Tray — documents only ("Skim all" goes straight to review)
  def documents_only
    render Campbooks::SkimTray.new(rings: [], doc_count: 3)
  end

  # @label Ring — theme
  def ring
    render Campbooks::SkimRing.new(theme: :important, label: "Alerts", count: 3)
  end

  # @label Ring — "Skim all" lead
  def all_ring
    render Campbooks::SkimRing.new(theme: nil, label: "Skim all", count: 217)
  end

  # @label Ring — Priority lane
  def priority_ring
    render Campbooks::SkimRing.new(theme: :priority, label: "Priority", count: 4)
  end

  # @label Ring — large count (abbreviated to 1.4k)
  def large_count
    render Campbooks::SkimRing.new(theme: :promotions, label: "Promotions", count: 1381)
  end

  # @label Tray — every theme
  def all_themes
    rings = (Emails::SkimBuilder::THEME_ORDER - [ :priority ]).each_with_index.map do |theme, i|
      { theme: theme, label: Emails::SkimBuilder::THEME_LABELS[theme], count: (i + 1) * 7 }
    end
    render Campbooks::SkimTray.new(rings: rings)
  end
end
