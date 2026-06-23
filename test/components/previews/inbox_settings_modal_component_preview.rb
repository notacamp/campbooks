class InboxSettingsModalComponentPreview < ViewComponent::Preview
  # The two-pane inbox settings dialog, shown open. The content pane lazy-loads
  # each section into a Turbo Frame in the running app; in the preview it shows
  # the loading placeholder.
  def default
    render(Campbooks::InboxSettingsModal.new(open: true))
  end

  # Opened to a specific section (Document types nav item highlighted).
  def document_types_active
    render(Campbooks::InboxSettingsModal.new(open: true, default_section: "document_types"))
  end

  # Closed — renders the dialog element with no `open` attribute (nothing visible).
  def closed
    render(Campbooks::InboxSettingsModal.new(open: false))
  end
end
