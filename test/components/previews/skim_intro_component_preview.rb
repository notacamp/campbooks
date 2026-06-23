# frozen_string_literal: true

class SkimIntroComponentPreview < ViewComponent::Preview
  # The intro normally sits as an absolute layer over a skim stack; the standalone
  # variants force it relative (!relative !inset-auto) into a fixed-height box so
  # Lookbook can show it on its own. `in_context` shows the real layered usage.

  # @label Email skim — first-run intro
  def email
    render Campbooks::SkimIntro.new(
      title: "Skim your inbox",
      lead: "The fastest way to clear your inbox — one stack at a time, like Stories for your email.",
      steps: [
        { icon: :swipe, label: "Tap the sides or swipe to step through your inbox, newest first." },
        { icon: :act,   label: "Keep, archive, or pin each stack with the buttons — no stack left behind." },
        { icon: :undo,  label: "Nothing is deleted: archived mail stays searchable, and every action can be undone." }
      ],
      cta: "Start skimming",
      dismiss_action: "skim-mode#dismissIntro",
      class: "!relative !inset-auto mx-auto h-[620px] max-w-md overflow-hidden rounded-2xl border border-border"
    )
  end

  # @label Document skim — first-run intro
  def document
    render Campbooks::SkimIntro.new(
      title: "Skim your documents",
      lead: "Clear your review queue one document at a time — confirm what the AI filed, fix what it missed.",
      steps: [
        { icon: :swipe,   label: "Tap the sides or swipe to step through the documents to review." },
        { icon: :approve, label: "Approve the AI's filing, or fix it — reclassify, edit, or reprocess." },
        { icon: :undo,    label: "Approvals are undoable — nothing is locked in." }
      ],
      cta: "Start reviewing",
      dismiss_action: "doc-skim-mode#dismissIntro",
      class: "!relative !inset-auto mx-auto h-[620px] max-w-md overflow-hidden rounded-2xl border border-border"
    )
  end

  # @label In context — full email skim stack with the intro shown
  def in_context
    render Campbooks::SkimStack.new(rings: Campbooks::SkimCardsDemo::RINGS, standalone: true, show_intro: true)
  end
end
