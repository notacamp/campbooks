# frozen_string_literal: true

# Shown wherever an AI-dependent feature can't run for lack of a provider.
# Three variants (panel / banner / inline) x three capabilities (text / documents
# / embeddings). return_to is "/" here so the modal CTA has somewhere to go back to.
class AiSetupPromptComponentPreview < ViewComponent::Preview
  # === Panel — centered empty-state for a whole pane (Scout chat, compose, docs) ===

  def panel_text
    render Campbooks::AiSetupPrompt.new(capability: :text, variant: :panel, return_to: "/")
  end

  def panel_documents
    render Campbooks::AiSetupPrompt.new(capability: :documents, variant: :panel, return_to: "/")
  end

  def panel_embeddings
    render Campbooks::AiSetupPrompt.new(capability: :embeddings, variant: :panel, return_to: "/")
  end

  # === Banner — slim strip above a section ===

  def banner_text
    render Campbooks::AiSetupPrompt.new(capability: :text, variant: :banner, return_to: "/")
  end

  def banner_documents
    render Campbooks::AiSetupPrompt.new(capability: :documents, variant: :banner, return_to: "/")
  end

  def banner_embeddings
    render Campbooks::AiSetupPrompt.new(capability: :embeddings, variant: :banner, return_to: "/")
  end

  # === Inline — one compact line next to a disabled control ===

  def inline_text
    render Campbooks::AiSetupPrompt.new(capability: :text, variant: :inline, return_to: "/")
  end

  def inline_documents
    render Campbooks::AiSetupPrompt.new(capability: :documents, variant: :inline, return_to: "/")
  end

  def inline_embeddings
    render Campbooks::AiSetupPrompt.new(capability: :embeddings, variant: :inline, return_to: "/")
  end
end
