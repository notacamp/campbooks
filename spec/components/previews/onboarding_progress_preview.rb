# frozen_string_literal: true

class OnboardingProgressPreview < Lookbook::Preview
  STEPS = %w[workspace email_accounts ai_configuration classification review].freeze

  def first_step
    render(Campbooks::OnboardingProgress.new(
      current_step: "workspace",
      previous_step: nil
    ))
  end

  def middle_step
    render(Campbooks::OnboardingProgress.new(
      current_step: "ai_configuration",
      previous_step: "email_accounts"
    ))
  end

  def last_step
    render(Campbooks::OnboardingProgress.new(
      current_step: "review",
      previous_step: "classification"
    ))
  end
end
