# frozen_string_literal: true

class UserMenuComponentPreview < ViewComponent::Preview
  # @label Popover (desktop, beside the rail) — open
  def popover
    render Campbooks::UserMenu.new(variant: :popover, open: true)
  end

  # @label Sheet (mobile, bottom) — open
  def sheet
    render Campbooks::UserMenu.new(variant: :sheet, open: true)
  end
end
