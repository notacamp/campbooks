# frozen_string_literal: true

module Campbooks
  # Generic bar-chart sparkline rendered as an inline SVG. Each point gets one
  # 8-unit wide slot (6-unit bar + 2-unit gap). Values are baseline-anchored so
  # the tallest bar fills the full height. Error emphasis is drawn at the bottom
  # of each bar in the destructive color; the neutral portion sits above it with
  # a 1-unit gap.
  #
  # No JavaScript is needed: native SVG <title> elements provide tooltips, and
  # the transparent hover rects give a wide hit target per slot.
  #
  # Usage:
  #   Campbooks::Sparkline.new(
  #     points: [{ value: 42, emphasis: 3, label: "Jan 1 · 42 calls, 3 errors" }, ...],
  #     height: 32,
  #     label:  "Activity over the last 24 hours"
  #   )
  class Sparkline < Campbooks::Base
    # @param points  [Array<Hash>] each: { value: Integer, emphasis: Integer, label: String }
    # @param height  [Integer] SVG viewBox height in units
    # @param label   [String, nil] aria-label for the <svg>
    def initialize(points:, height: 32, label: nil, **attrs)
      @points = points
      @height = height
      @label  = label
      @attrs  = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      merged = class_names("w-full", custom_class)
      raw safe(build_svg(merged))
    end

    private

    def build_svg(css_class)
      max_val  = @points.map { |p| p[:value].to_i }.max.to_f
      max_val  = 1.0 if max_val.zero?
      width    = @points.size * 8

      aria = @label ? %( aria-label="#{CGI.escapeHTML(@label)}") : ""
      klass = css_class.present? ? %( class="#{CGI.escapeHTML(css_class)}") : ""

      parts  = []
      parts << %(<svg role="img"#{aria}#{klass} preserveAspectRatio="none" viewBox="0 0 #{width} #{@height}">)

      neutral_rects  = []
      emphasis_rects = []
      hover_rects    = []

      @points.each_with_index do |point, i|
        x   = i * 8
        val = point[:value].to_i
        emp = [ point[:emphasis].to_i, val ].min
        neu = val - emp

        if val.zero?
          # Zero-value: baseline stub so strip reads as N slots.
          neutral_rects << %(<rect x="#{x}" y="#{"%.4g" % (@height - 1.5)}" width="6" height="1.5" rx="1"/>)
        else
          # Total bar height, minimum 2 so non-zero values are always visible.
          total_h = [ (@height.to_f * val / max_val).round, 2 ].max

          if emp > 0 && neu > 0
            # Both portions with a 1-unit gap between them.
            emp_h = [ (@height.to_f * emp / max_val).round, 1 ].max
            neu_h = [ total_h - emp_h - 1, 1 ].max
            emp_y = @height - emp_h
            neu_y = emp_y - 1 - neu_h
            neutral_rects  << %(<rect x="#{x}" y="#{neu_y}" width="6" height="#{neu_h}" rx="1"/>)
            emphasis_rects << %(<rect x="#{x}" y="#{emp_y}" width="6" height="#{emp_h}" rx="1"/>)
          elsif emp > 0
            emp_y = @height - total_h
            emphasis_rects << %(<rect x="#{x}" y="#{emp_y}" width="6" height="#{total_h}" rx="1"/>)
          else
            neu_y = @height - total_h
            neutral_rects << %(<rect x="#{x}" y="#{neu_y}" width="6" height="#{total_h}" rx="1"/>)
          end
        end

        title_text = CGI.escapeHTML(point[:label].to_s)
        hover_rects << %(<rect x="#{x}" y="0" width="8" height="#{@height}" fill="transparent"><title>#{title_text}</title></rect>)
      end

      if neutral_rects.any?
        parts << %(<g fill="currentColor" class="text-gray-300 dark:text-gray-600">#{neutral_rects.join}</g>)
      end
      if emphasis_rects.any?
        parts << %(<g fill="currentColor" class="text-destructive">#{emphasis_rects.join}</g>)
      end
      parts << hover_rects.join
      parts << "</svg>"
      parts.join
    end
  end
end
