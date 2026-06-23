class AiSetupProposalComponentPreview < ViewComponent::Preview
  def document_types
    items = [
      { "name" => "client_invoice", "color" => "#3b82f6", "prompt" => "Invoices you send to clients for legal services rendered." },
      { "name" => "engagement_letter", "color" => "#8b5cf6", "prompt" => "Signed agreements defining the scope of a client engagement." },
      { "name" => "court_filing", "color" => "#ef4444", "prompt" => "Documents filed with, or received from, a court." }
    ]
    render Campbooks::AiSetupProposal.new(items: items, kind: "document_types", form_action: "#")
  end

  def tags
    items = [
      { "name" => "urgent", "color" => "#ef4444", "prompt" => "Anything needing a same-day response." },
      { "name" => "billing", "color" => "#22c55e", "prompt" => "Invoices, payments, and accounting threads." }
    ]
    render Campbooks::AiSetupProposal.new(items: items, kind: "tags", form_action: "#")
  end

  def empty
    render Campbooks::AiSetupProposal.new(items: [], kind: "tags", form_action: "#")
  end
end
