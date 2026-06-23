class ContactPillInputPreview < Lookbook::Preview
  def default
    render(Campbooks::ContactPillInput.new(name: "to_address", placeholder: "Search contacts..."))
  end

  def with_initial_value
    render(Campbooks::ContactPillInput.new(
      name: "cc_address",
      value: "john@example.com, jane@acme.com",
      placeholder: "Add CC recipients..."
    ))
  end

  def with_label
    render(Campbooks::ContactPillInput.new(
      name: "bcc_address",
      placeholder: "Add BCC recipients...",
      label: "Bcc:"
    ))
  end
end
