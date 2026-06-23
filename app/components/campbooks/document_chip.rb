# frozen_string_literal: true

module Campbooks
  class DocumentChip < Campbooks::Base
    # @param document [Document]
    def initialize(document:, **attrs)
      @document = document
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)

      a(
        href: helpers.document_path(@document),
        target: "_blank",
        class: class_names(
          "inline-flex items-center gap-1.5 text-[12px] text-gray-600 bg-card border border-gray-300 rounded-lg px-2.5 py-1 hover:bg-gray-50 hover:border-gray-400 transition-colors max-w-full",
          custom_class
        ),
        **@attrs
      ) do
        render_type_badge
        span(class: "truncate min-w-0 max-w-[160px]") { plain(@document.original_file.filename.to_s) }
        if @document.document_date
          span(class: "text-[10px] text-gray-400 flex-shrink-0") {
            plain(l(@document.document_date, format: :numeric))
          }
        end
      end
    end

    private

    def render_type_badge
      type = @document.document_type
      color = type&.respond_to?(:color) && type.color.presence || "#6b7280"
      label = type&.respond_to?(:name) ? type.name.humanize : t(".unclassified")

      span(
        class: "inline-flex items-center gap-1 rounded px-2 py-0.5 text-[11px] font-medium flex-shrink-0",
        style: "color: #{color}; background-color: #{color}20"
      ) do
        render(Campbooks::ColorDot.new(color: color, size: :sm))
        plain(label)
      end
    end
  end
end
