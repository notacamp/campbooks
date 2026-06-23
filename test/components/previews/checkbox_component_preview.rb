class CheckboxComponentPreview < ViewComponent::Preview
  # Unchecked checkbox with label
  def unchecked
    render Campbooks::Checkbox.new("agree", label: "I agree to the terms")
  end

  # Checked checkbox
  def checked
    render Campbooks::Checkbox.new("subscribe", label: "Subscribe to newsletter", checked: true)
  end

  # Disabled unchecked checkbox
  def disabled
    render Campbooks::Checkbox.new("feature", label: "Enable feature", disabled: true)
  end

  # Disabled checked checkbox
  def disabled_checked
    render Campbooks::Checkbox.new("legacy", label: "Legacy mode", checked: true, disabled: true)
  end
end
