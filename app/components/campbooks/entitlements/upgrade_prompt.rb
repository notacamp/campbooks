# frozen_string_literal: true

module Campbooks
  module Entitlements
    # Shown wherever an action is blocked by the workspace's plan — the feature
    # isn't included, has been toggled off, or a usage limit is reached. Hands the
    # user one clear path: view/upgrade the plan. Mirrors Campbooks::AiSetupPrompt's
    # panel / banner / inline variants.
    #
    #   render Campbooks::Entitlements::UpgradePrompt.new(feature: :workflows, reason: :not_allowed)
    #
    # reason  — :not_allowed | :not_enabled | :over_limit (drives the body copy)
    # variant — :panel (whole pane) | :banner (slim strip) | :inline (one line)
    class UpgradePrompt < Campbooks::Base
      VARIANTS = %i[panel banner inline].freeze
      REASONS = %i[not_allowed not_enabled over_limit].freeze

      LOCK_PATH =
        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" ' \
        'd="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"/>'

      def initialize(feature:, reason: :not_allowed, variant: :panel, **attrs)
        @feature = feature.to_s
        @reason = REASONS.include?(reason&.to_sym) ? reason.to_sym : :not_allowed
        @variant = VARIANTS.include?(variant&.to_sym) ? variant.to_sym : :panel
        @attrs = attrs
      end

      def view_template
        case @variant
        when :banner then banner_template
        when :inline then inline_template
        else panel_template
        end
      end

      private

      def panel_template
        div(class: class_names("px-6 py-10 sm:py-14 text-center", @attrs.delete(:class)), role: "status", **@attrs) do
          div(class: "mx-auto max-w-sm") do
            div(class: "flex justify-center") { lock_icon("w-10 h-10 text-muted-foreground") }
            h2(class: "mt-4 text-base font-semibold tracking-tight text-foreground") { title }
            p(class: "mt-1.5 text-sm leading-relaxed text-muted-foreground") { body }
            div(class: "mt-5 flex justify-center") { cta(size: :md) }
          end
        end
      end

      def banner_template
        div(
          class: class_names(
            "flex flex-col gap-3 rounded-xl border border-border bg-muted px-4 py-3",
            "sm:flex-row sm:items-center sm:justify-between sm:gap-4",
            @attrs.delete(:class)
          ),
          role: "status", **@attrs
        ) do
          div(class: "flex items-start gap-3 sm:items-center") do
            span(class: "mt-0.5 sm:mt-0 shrink-0") { lock_icon("w-5 h-5 text-muted-foreground") }
            div(class: "min-w-0") do
              p(class: "text-sm font-medium text-foreground") { title }
              p(class: "text-[13px] leading-relaxed text-muted-foreground") { body }
            end
          end
          div(class: "shrink-0") { cta(size: :sm) }
        end
      end

      def inline_template
        p(class: class_names("flex flex-wrap items-center gap-x-1.5 gap-y-1 text-sm text-muted-foreground", @attrs.delete(:class)), role: "status", **@attrs) do
          plain body
          whitespace
          a(href: helpers.settings_plan_path,
            class: "font-medium text-foreground underline underline-offset-2 hover:text-foreground/80") { cta_label }
        end
      end

      def cta(size:)
        render Campbooks::Button.new(variant: :primary, size: size, href: helpers.settings_plan_path) { cta_label }
      end

      def title = t(".title", feature: feature_label)
      def body = t(".reason.#{@reason}", feature: feature_label)
      def cta_label = t(".cta")
      def feature_label = t("entitlements.features.#{@feature}")

      def lock_icon(classes)
        svg(class: classes, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
          raw(safe(LOCK_PATH))
        end
      end
    end
  end
end
