# frozen_string_literal: true

module Campbooks
  # Card for the organizations index grid. Shows the org name, member count, domain,
  # and email count badge. Links to the organization show page.
  class OrganizationCard < Campbooks::Base
    def initialize(organization:, **attrs)
      @org = organization
      @attrs = attrs
    end

    def view_template
      a(
        href: helpers.organization_path(@org),
        class: class_names(
          "flex items-center gap-3 rounded-xl border border-border bg-card p-4 min-w-0",
          "hover:bg-muted/40 transition-colors",
          @attrs.delete(:class)
        ),
        **@attrs
      ) do
        icon
        info
        email_badge if @org.email_count > 0
      end
    end

    private

    def icon
      div(class: "flex size-10 items-center justify-center rounded-lg bg-muted text-muted-foreground flex-shrink-0") do
        svg(class: "w-5 h-5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do |s|
          s.path(
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "stroke-width": "1.9",
            d: "M3 21h18M3 10h18M5 6l7-3 7 3M4 10v11m16-11v11M8 14v.01M12 14v.01M16 14v.01M8 18v.01M12 18v.01M16 18v.01"
          )
        end
      end
    end

    def info
      div(class: "min-w-0 flex-1") do
        div(class: "text-sm font-semibold text-foreground truncate") { plain(@org.name) }
        div(class: "text-xs text-muted-foreground mt-0.5") do
          plain(t("components.organization_card.members", count: @org.member_count))
          if @org.domain.present?
            plain(" · #{@org.domain}")
          end
        end
      end
    end

    def email_badge
      span(class: "inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs font-medium text-muted-foreground flex-shrink-0") do
        plain(@org.email_count.to_s)
      end
    end
  end
end
