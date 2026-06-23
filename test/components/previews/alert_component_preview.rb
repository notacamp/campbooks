class AlertComponentPreview < ViewComponent::Preview
  # Canonical variants
  def success
    render Campbooks::Alert.new(variant: :success, message: "Document was successfully uploaded.")
  end

  def error
    render Campbooks::Alert.new(variant: :error, message: "Something went wrong. Please try again.")
  end

  def warning
    render Campbooks::Alert.new(variant: :warning, message: "Please review this document before approving.")
  end

  def info
    render Campbooks::Alert.new(variant: :info, message: "A new version of the report is available.")
  end

  # Legacy aliases (notice → success, alert → error) kept for back-compat
  def notice_alias
    render Campbooks::Alert.new(variant: :notice, message: "Saved. (notice → success alias)")
  end

  def alert_alias
    render Campbooks::Alert.new(variant: :alert, message: "Failed. (alert → error alias)")
  end

  def as_block
    render Campbooks::Alert.new(variant: :info) do
      "Custom block content with arbitrary inner HTML."
    end
  end
end
