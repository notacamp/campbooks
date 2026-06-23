class BugReportButtonComponentPreview < ViewComponent::Preview
  # The trigger as it sits in the desktop nav rail footer.
  def rail
    render(Campbooks::BugReportButton.new)
  end

  # The trigger sized for the mobile topbar.
  def topbar
    render(Campbooks::BugReportButton.new(
      class: "inline-flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground cursor-pointer"
    ))
  end
end
