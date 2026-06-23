# frozen_string_literal: true

module Campbooks
  # The /documents Skim tray: a horizontally-scrollable row of document-category
  # rings (plus a leading "Review all"), pinned above the documents list. Tapping a
  # ring opens the Skim viewer at that category; "Review all" runs every ring in
  # sequence. Renders nothing when there is nothing to review. The document-world
  # analogue of Campbooks::SkimTray.
  class DocSkimTray < Campbooks::Base
    def initialize(rings:, **attrs)
      @rings = rings || []
      @attrs = attrs
    end

    def view_template
      return if @rings.empty?

      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "flex items-start gap-2.5 overflow-x-auto px-3 py-2.5",
          "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          custom
        ),
        **@attrs
      ) do
        render Campbooks::DocSkimRing.new(
          category: nil, label: t(".review_all"), count: @rings.sum { |ring| ring[:count].to_i },
          data: { action: "click->doc-skim-overlay#open" }
        )
        @rings.each do |ring|
          render Campbooks::DocSkimRing.new(
            category: ring[:category],
            label: ring[:label],
            count: ring[:count],
            data: { action: "click->doc-skim-overlay#openTo", doc_skim_overlay_category_param: ring[:category] }
          )
        end
      end
    end
  end
end
