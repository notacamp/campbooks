# frozen_string_literal: true

module Campbooks
  class OnboardingProgress < Campbooks::Base
    STEPS = %w[workspace email_accounts ai_configuration classification review].freeze

    def initialize(current_step:, previous_step: nil, **attrs)
      @current_step = current_step
      @previous_step = previous_step
      @attrs = attrs
    end

    def view_template
      div(class: "mb-8", **@attrs) do
        div(class: "flex items-center justify-between gap-3 mb-4") do
          render_back_link
          render_skip_link
        end
        render_progress
        render_skip_hint
      end
    end

    private

    def render_back_link
      if @previous_step
        a(href: helpers.onboarding_path(step: @previous_step),
          class: "inline-flex items-center gap-1 text-sm text-gray-500 hover:text-gray-900 transition-colors") do
          raw(safe(%(<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/></svg>)))
          plain(t("shared.actions.back"))
        end
      else
        # Spacer so the skip link stays right-aligned on the first step.
        span
      end
    end

    # Always-visible escape hatch, styled as a clear (but secondary) button so
    # it's obvious that setup is optional — not a faint link users miss. A native
    # submit (turbo: false) is required: snooze redirects to root (a different
    # layout) which a Turbo form submission silently fails to render, leaving the
    # user stuck on the wizard.
    def render_skip_link
      raw(safe(helpers.button_to(
        "#{t("shared.actions.skip_for_now")} →",
        helpers.snooze_onboarding_path,
        method: :post,
        form: { data: { turbo: false } },
        class: "inline-flex items-center gap-1 rounded-full border border-gray-300 bg-white px-3.5 py-1.5 text-sm font-medium text-gray-600 hover:bg-gray-50 hover:text-gray-900 hover:border-gray-400 transition-colors cursor-pointer"
      )))
    end

    # Reassurance that nothing in the wizard is mandatory (feedback 2026-06-22).
    def render_skip_hint
      p(class: "mt-3 text-center text-xs text-gray-500") { t(".skip_hint") }
    end

    def step_labels
      {
        "workspace" => t(".steps.workspace"),
        "email_accounts" => t(".steps.email_accounts"),
        "ai_configuration" => t(".steps.ai_configuration"),
        "classification" => t(".steps.classification"),
        "review" => t(".steps.review")
      }
    end

    def render_progress
      step_index = STEPS.index(@current_step) || 0
      labels = step_labels

      steps = STEPS.map.with_index do |step, i|
        status = if i < step_index then :completed
        elsif i == step_index then :current
        else :pending
        end
        { label: labels[step] || step.titleize, status: status }
      end

      render(Campbooks::ProgressIndicator.new(steps: steps, class: "max-w-2xl mx-auto"))
    end
  end
end
