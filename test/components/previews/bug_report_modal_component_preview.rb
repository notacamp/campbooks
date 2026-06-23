class BugReportModalComponentPreview < ViewComponent::Preview
  # The dialog in its open state. In the app it renders hidden until a
  # [data-bug-report-open] trigger fires; `open: true` forces it visible so the
  # form, screenshot opt-in, and context note can be reviewed here.
  def open
    render(Campbooks::BugReportModal.new(open: true))
  end

  # The default (hidden) render — present in the DOM, waiting for a trigger.
  def closed
    render(Campbooks::BugReportModal.new)
  end
end
