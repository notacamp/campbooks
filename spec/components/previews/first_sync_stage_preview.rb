# frozen_string_literal: true

class FirstSyncStagePreview < Lookbook::Preview
  # Waiting state — account connected, queue not yet picked up.
  def waiting
    render(Campbooks::FirstSyncStage.new(
      status: { state: :waiting, found: 0, sorted: 0, needs_you: 0 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/"
    ))
  end

  # Scanning — a first sync is in flight; counters tick up as mail lands.
  def scanning
    render(Campbooks::FirstSyncStage.new(
      status: { state: :scanning, found: 42, sorted: 38, needs_you: 3 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/"
    ))
  end

  # Scanning with persona card — the in-fill question shown to new users.
  def scanning_with_persona
    render(Campbooks::FirstSyncStage.new(
      status: { state: :scanning, found: 42, sorted: 38, needs_you: 3 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/",
      persona_url: "/onboarding/apply_persona",
      skip_url: "/onboarding/skip_first_sync"
    ))
  end

  # Waiting with persona card — the very first render before scanning has started.
  def waiting_with_persona
    render(Campbooks::FirstSyncStage.new(
      status: { state: :waiting, found: 0, sorted: 0, needs_you: 0 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/",
      persona_url: "/onboarding/apply_persona",
      skip_url: "/onboarding/skip_first_sync"
    ))
  end

  # Done — scan completed with mail found; halo swapped for check, CTA revealed.
  def done
    render(Campbooks::FirstSyncStage.new(
      status: { state: :done, found: 124, sorted: 118, needs_you: 7 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/"
    ))
  end

  # Error — every attempt failed; amber note + reconnect link shown.
  def error
    render(Campbooks::FirstSyncStage.new(
      status: { state: :error, found: 0, sorted: 0, needs_you: 0 },
      status_url: "/onboarding/first_sync_status",
      inbox_path: "/email_messages",
      feed_path: "/"
    ))
  end
end
