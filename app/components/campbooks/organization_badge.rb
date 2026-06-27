# frozen_string_literal: true

module Campbooks
  # A small chip showing an organization's name, optionally linked to its show page.
  # Used on contact profiles, email detail, and document pages to surface the
  # company affiliation. Suppressed entirely when the feature flag is off.
  class OrganizationBadge < Campbooks::Base
    def initialize(organization:, linked: true, **attrs)
      @organization = organization
      @linked = linked
      @attrs = attrs
    end

    def view_template
      return unless Features.organizations?
      return if @organization.nil?

      if @linked && @organization.persisted?
        a(
          href: helpers.organization_path(@organization),
          class: "inline-flex no-underline hover:opacity-80",
          **@attrs
        ) { badge_content }
      else
        span(class: "inline-flex", **@attrs) { badge_content }
      end
    end

    private

    def badge_content
      div(class: "inline-flex items-center gap-1 rounded-md bg-muted/60 border border-border/50 px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground") do
        svg(
          class: "w-3 h-3 flex-shrink-0",
          fill: "none",
          stroke: "currentColor",
          viewBox: "0 0 24 24"
        ) do |s|
          s.path(
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "stroke-width": "2",
            d: "M3 21h18M3 10h18M5 6l7-3 7 3M4 10v11m16-11v11M8 14v.01M12 14v.01M16 14v.01M8 18v.01M12 18v.01M16 18v.01"
          )
        end
        span(class: "truncate") { plain(@organization.name) }
      end
    end
  end
end
