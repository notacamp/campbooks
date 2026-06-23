# frozen_string_literal: true

module Campbooks
  module Entitlements
    # Small chip showing a workspace's current plan (Free / Pro / Business / …).
    #
    #   render Campbooks::Entitlements::PlanBadge.new(plan: "pro")
    class PlanBadge < Campbooks::Base
      VARIANTS = {
        "free"      => :neutral,
        "pro"       => :accent,
        "business"  => :info,
        "unlimited" => :success
      }.freeze

      def initialize(plan:, size: :md, **attrs)
        @plan = plan.to_s
        @size = size
        @attrs = attrs
      end

      def view_template
        render Campbooks::Badge.new(variant: VARIANTS.fetch(@plan, :neutral), size: @size, **@attrs) do
          t(".names.#{@plan}")
        end
      end
    end
  end
end
