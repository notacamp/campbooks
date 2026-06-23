# frozen_string_literal: true

module Campbooks
  # One row on Settings → Security: a factor's icon, title, On/Off status badge,
  # and description, with a yielded body for its actions (buttons) or details
  # (e.g. the list of registered passkeys).
  class SecurityMethodCard < Campbooks::Base
    # @param title [String]
    # @param enabled [Boolean] drives the On/Off status badge
    # @param description [String, nil]
    # @param icon [String, nil] SVG path data for the leading icon tile
    def initialize(title:, enabled: false, description: nil, icon: nil, **attrs)
      @title = title
      @enabled = enabled
      @description = description
      @icon = icon
      @attrs = attrs
    end

    def view_template(&content)
      body = content ? capture(&content) : nil
      custom_class = @attrs.delete(:class)

      div(class: class_names("bg-card rounded-xl shadow-sm border border-border p-5 sm:p-6", custom_class), **@attrs) do
        div(class: "flex items-start gap-3") do
          icon_tile if @icon
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-center gap-2 flex-wrap") do
              h2(class: "text-base font-semibold text-foreground") { @title }
              render(Campbooks::Badge.new(variant: @enabled ? :success : :neutral)) do
                @enabled ? t(".enabled") : t(".disabled")
              end
            end
            p(class: "text-sm text-muted-foreground mt-1") { @description } if @description.present?
          end
        end

        div(class: "mt-4") { raw(safe(body)) } if body.present?
      end
    end

    private

    def icon_tile
      div(class: "flex-shrink-0 w-9 h-9 rounded-lg bg-muted text-muted-foreground flex items-center justify-center") do
        raw(safe(%(<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="#{@icon}"/></svg>)))
      end
    end
  end
end
