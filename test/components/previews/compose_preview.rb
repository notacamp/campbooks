# Compose-revamp pieces that render standalone. The Engine and Dock shells are
# page-scale surfaces wired to the session (entitlements, autosave routes,
# live TipTap), so — like the old inline composer — they're exercised in the
# app, not in Lookbook.
class ComposePreview < Lookbook::Preview
  # The parked-draft pill: bottom-right capsule that survives navigation and
  # resumes the draft in the Dock.
  def pill
    render(Campbooks::Compose::Pill.new(draft: DraftEmail.new(id: SecureRandom.uuid, subject: "Re: Q3 budget review")))
  end

  # A pill for a draft with no subject yet — falls back to the first recipient.
  def pill_untitled
    render(Campbooks::Compose::Pill.new(draft: DraftEmail.new(id: SecureRandom.uuid, to_address: "ana@acme.com")))
  end

  # Scout's ghost draft (Ember-glass block) as it appears above the canvas:
  # take ownership, retone, or start blank.
  def scout_ghost_draft
    render(Campbooks::Compose::ScoutDraft.new(
      text: "Hi Ana, yes, confirmed. The Q3 numbers are final on my side. " \
            "One caveat: contractor spend is still moving and I will have it " \
            "locked by Thursday, ahead of your Friday deadline.",
      message: nil
    ))
  end

  # The toolbar-less editor used by both compose shells — select text to see
  # the floating formatting bubble.
  def bubble_editor
    render(Campbooks::RichTextEditor.new(
      input_name: "body",
      content: "<p>Select some of this text to try the formatting bubble.</p>",
      toolbar: false,
      bubble: true,
      frameless: true,
      min_height: "160px"
    ))
  end
end
