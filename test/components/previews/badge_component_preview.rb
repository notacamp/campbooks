# frozen_string_literal: true

class BadgeComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Badge.new { "Default" })
  end

  def variants_md
    render(Campbooks::BadgeVariants.new(size: :md))
  end

  def variants_sm
    render(Campbooks::BadgeVariants.new(size: :sm))
  end

  def neutral
    render(Campbooks::Badge.new(variant: :neutral) { "Neutral" })
  end

  def accent
    render(Campbooks::Badge.new(variant: :accent) { "Accent" })
  end

  def success
    render(Campbooks::Badge.new(variant: :success) { "Success" })
  end

  def warning
    render(Campbooks::Badge.new(variant: :warning) { "Warning" })
  end

  def danger
    render(Campbooks::Badge.new(variant: :danger) { "Danger" })
  end

  def info
    render(Campbooks::Badge.new(variant: :info) { "Info" })
  end

  def count
    render(Campbooks::Badge.new(variant: :accent, size: :sm) { "42" })
  end
end
