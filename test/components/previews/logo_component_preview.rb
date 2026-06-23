# frozen_string_literal: true

class LogoComponentPreview < ViewComponent::Preview
  # Full lockup: mark + wordmark
  def default
    render(Campbooks::Logo.new(size: :md))
  end

  def small
    render(Campbooks::Logo.new(size: :sm))
  end

  def large
    render(Campbooks::Logo.new(size: :lg))
  end

  # Mark only (no wordmark) — used for favicons, avatars, compact spots
  def mark_only
    render(Campbooks::Logo.new(size: :lg, variant: :mark))
  end

  # Full lockup with the Beta tag (cloud builds pass `beta: !self_hosted?`)
  def beta
    render(Campbooks::Logo.new(size: :lg, beta: true))
  end

  # Mark + Beta tag — the desktop nav rail variant
  def beta_mark
    render(Campbooks::Logo.new(size: :md, variant: :mark, beta: true))
  end
end
