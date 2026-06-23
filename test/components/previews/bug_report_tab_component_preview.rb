class BugReportTabComponentPreview < ViewComponent::Preview
  # The floating "Report a bug" tab, pinned to the right edge of the viewport
  # (Hotjar-style). In the app it opens the BugReportModal in place.
  def default
    render(Campbooks::BugReportTab.new)
  end
end
