# frozen_string_literal: true

module Campbooks
  module Entitlements
    # The Settings → Plan page body: the current plan + a per-feature breakdown of
    # what it includes (flags), the limits and live usage (limits), and config
    # knobs (e.g. AI model tier), with an over-cap warning after a downgrade.
    #
    #   render Campbooks::Entitlements::PlanSummary.new(entitlements: current_entitlements)
    #
    # Deferred (not-yet-metered) limit features are hidden — a limit feature with
    # a nil usage isn't enforced yet, so it would only confuse.
    class PlanSummary < Campbooks::Base
      def initialize(entitlements:, **attrs)
        @entitlements = entitlements
        @attrs = attrs
      end

      def view_template
        div(class: class_names("space-y-6", @attrs.delete(:class)), **@attrs) do
          header
          if rows.empty?
            self_hosted_notice
          else
            feature_list
          end
        end
      end

      private

      def header
        div(class: "flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between") do
          div do
            p(class: "text-sm text-muted-foreground") { t(".current_plan") }
            div(class: "mt-1") { render Campbooks::Entitlements::PlanBadge.new(plan: @entitlements.plan_name, size: :md) }
          end
        end
      end

      def self_hosted_notice
        render Campbooks::Card.new(padding: :lg) do
          h3(class: "text-sm font-semibold text-foreground") { t(".self_hosted_title") }
          p(class: "mt-1 text-sm text-muted-foreground") { t(".self_hosted_body") }
        end
      end

      def feature_list
        render Campbooks::Card.new(padding: :none) do
          ul(class: "divide-y divide-border") do
            rows.each { |key, row| feature_row(key, row) }
          end
        end
      end

      def feature_row(key, row)
        li(class: "flex flex-col gap-1.5 px-4 py-3.5 sm:flex-row sm:items-center sm:justify-between sm:gap-4") do
          div(class: "min-w-0") do
            p(class: "text-sm font-medium text-foreground") { t("entitlements.features.#{key}") }
            if row[:over_cap]
              p(class: "mt-0.5 text-xs text-amber-600 dark:text-amber-500") do
                t(".over_cap", usage: row[:usage], limit: row[:limit])
              end
            end
          end
          div(class: "shrink-0") { value_for(row) }
        end
      end

      def value_for(row)
        case row[:type]
        when :flag
          render Campbooks::Badge.new(variant: row[:active] ? :success : :neutral, size: :md) do
            row[:active] ? t(".included") : t(".not_included")
          end
        when :config
          span(class: "text-sm text-foreground") { tier_label(row.dig(:config, :tier)) }
        else
          span(class: "text-sm text-foreground") { limit_value(row) }
        end
      end

      def limit_value(row)
        return t(".not_included") unless row[:active]
        return t(".unlimited") if row[:limit].nil?
        return t(".usage_of_limit", usage: row[:usage], limit: row[:limit]) unless row[:usage].nil?

        row[:limit].to_s
      end

      def tier_label(tier)
        return t(".unlimited") if tier.blank?

        t(".tiers.#{tier}", default: tier.to_s.titleize)
      end

      # Hide deferred (un-metered) limit features: a :limit row with nil usage is
      # not enforced yet.
      def rows
        @rows ||= @entitlements.summary.reject { |_key, row| row[:type] == :limit && row[:usage].nil? }
      end
    end
  end
end
