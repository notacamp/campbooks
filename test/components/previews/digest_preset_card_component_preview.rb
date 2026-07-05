# frozen_string_literal: true

# Previews for the digest preset gallery cards (/digests/new).
class DigestPresetCardComponentPreview < ViewComponent::Preview
  # The full gallery grid, as rendered on /digests/new.
  def gallery
    render_with_template
  end

  # A single preset card.
  def single
    render Campbooks::Digests::PresetCard.new(preset: Digests::Presets.find("week_ahead"))
  end

  # The custom (build-your-own) card that closes the gallery.
  def custom
    render Campbooks::Digests::PresetCard.new(preset: Digests::Presets.find("custom"))
  end
end
