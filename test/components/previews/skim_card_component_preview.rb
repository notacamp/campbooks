# frozen_string_literal: true

class SkimCardComponentPreview < ViewComponent::Preview
  # @label Story flow (a few clusters)
  def gallery
    render Campbooks::SkimCardsDemo.new
  end

  # @label Skim Mode — Stories viewer (full-screen; ← back / → keep / E archive)
  def story
    render Campbooks::SkimStack.new(rings: Campbooks::SkimCardsDemo::RINGS, standalone: true)
  end

  # @label Story frame (tall fill layout, email list expanded by default)
  def frame
    render Campbooks::SkimCard.new(
      category: :personal, title: "3 messages from Jamie", count: 3, fill: true, show_progress: false,
      bucket_label: "Today",
      summary: "She is asking about lunch tomorrow and shared a Figma file to review.",
      emails: [
        { id: 1, sender: "Jamie", subject: "Lunch tomorrow?", snippet: "Are you free around 1pm? I found a new spot near the studio." },
        { id: 2, sender: "Jamie", subject: "Carte de visite Jamie", snippet: "Sharing my new card design — let me know what you think." },
        { id: 3, sender: "Jamie", subject: "Reminder: edit the file", snippet: "Pushed the latest frames to Figma; your section is the last one." }
      ],
      class: "max-w-md"
    )
  end

  # @label Noise cluster (Keep / Archive all)
  def notifications_cluster
    render Campbooks::SkimCard.new(
      category: :notifications, title: "47 CircleCI builds", count: 47, position: 2, total: 8,
      bucket_label: "Today",
      summary: "All resolved — every branch already merged. Nothing here needs you.",
      samples: [ "Workflow failed: connect-backend", "Workflow failed: august", "Workflow canceled: yale-home" ]
    )
  end

  # @label Cluster with AI summary (Scout's "what is this about", fill layout)
  def scout_cluster_summary
    render Campbooks::SkimCard.new(
      category: :notifications, title: "12 from GitHub", count: 12, fill: true, show_progress: false,
      bucket_label: "Today",
      summary: "Mostly CI runs on the payments branch — three failed and need a look, the rest are merge notifications.",
      summary_digest: "demo123",
      emails: [
        { id: 1, sender: "GitHub", subject: "[org/api] CI failed on payments", snippet: "Workflow 'deploy' failed at step build." },
        { id: 2, sender: "GitHub", subject: "[org/web] PR #482 merged", snippet: "Your pull request was merged into main." },
        { id: 3, sender: "GitHub", subject: "[org/api] Re: review requested", snippet: "Ana requested your review on #488." }
      ]
    )
  end

  # @label Scout suggestion — learned "you usually archive these" (primary + cue)
  def scout_suggested_archive
    render Campbooks::SkimCard.new(
      category: :notifications, title: "12 from GitHub", count: 12, position: 2, total: 8,
      bucket_label: "Today",
      summary: "Pull-request and CI notifications across your repos.",
      scout_suggestion: { action: "archive", count: 9, total: 12 },
      samples: [ "[org/api] PR #482 merged", "[org/web] CI passed on main", "[org/api] Re: review requested" ]
    )
  end

  # @label Scout suggestion — learned "you usually pin these to Priority"
  def scout_suggested_promote
    render Campbooks::SkimCard.new(
      category: :important, title: "Invoice from EDP", count: 1, position: 1, total: 8,
      bucket_label: "Today",
      summary: "Your January electricity invoice — €84, due Jan 31.",
      scout_suggestion: { action: "promote", count: 4, total: 5 }
    )
  end

  # @label Suggested priority (confirmable cue)
  def suggested_priority
    render Campbooks::SkimCard.new(
      category: :important, title: "Banco BPI — verification code", count: 1, position: 1, total: 8,
      bucket_label: "Today", priority_suggested: true,
      summary: "A one-time code to confirm your transfer. Expires in 10 minutes."
    )
  end

  # @label Pinned (Priority lane)
  def pinned
    render Campbooks::SkimCard.new(
      category: :personal, title: "Jamie — Figma review", count: 1, position: 1, total: 4,
      bucket_label: "Yesterday", pinned: true,
      summary: "You pinned this to deal with it."
    )
  end

  # @label Pending sender (whitelist — Allow / Deny)
  def pending_sender
    render Campbooks::SkimCard.new(
      category: :promotions, title: "news@acme.example", count: 2, position: 1, total: 3,
      theme: :pending, bucket_label: "Today",
      summary: "A new sender — allow or deny them.",
      emails: [ { id: 1, sender: "Acme", subject: "Your weekly digest", snippet: "This week's highlights and a few picks for you." } ]
    )
  end

  # @label Starred sender (promoted, never grouped)
  def starred_sender
    render Campbooks::SkimCard.new(
      category: :personal, title: "Contract draft for review", count: 1, position: 1, total: 2,
      theme: :starred, bucket_label: "Today",
      summary: "From a sender you've starred.",
      emails: [ { id: 1, sender: "Jamie (starred)", subject: "Contract draft for review", snippet: "Final version attached — can you sign by Friday?" } ]
    )
  end

  # @label Follow-up (you replied, no answer — Draft follow-up / Dismiss)
  def follow_up
    render Campbooks::SkimCard.new(
      category: :personal, title: "Re: Spring charter — final balance", count: 1, fill: true, show_progress: false,
      theme: :follow_ups, bucket_label: "Last week",
      summary: "You replied — no answer yet. Nudge them?",
      follow_up_reason: "You asked them to confirm the final balance.",
      emails: [ { id: 1, sender: "Maria Santos", subject: "Re: Spring charter — final balance", snippet: "Thanks for the quote — just need the final number before we book." } ]
    )
  end
end
