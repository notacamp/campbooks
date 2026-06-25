# frozen_string_literal: true

# Previews for the in-context "Processed by <provider> · <region>" provenance note.
class AiProvenanceNotePreview < ViewComponent::Preview
  def eu_provider
    render Campbooks::AiProvenanceNote.new(provenance: {
      "provider" => "mistral", "model" => "mistral-small-latest", "region" => "EU"
    })
  end

  def us_provider
    render Campbooks::AiProvenanceNote.new(provenance: {
      "provider" => "openai", "model" => "gpt-4o-mini", "region" => "US"
    })
  end

  def absent
    render Campbooks::AiProvenanceNote.new(provenance: {})
  end
end
