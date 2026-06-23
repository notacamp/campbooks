# frozen_string_literal: true

module Campbooks
  # Preview-only demo of the Skim-Mode story flow. Cards lead with WHEN they
  # arrived (the time bucket) since the menu is organised by THEME and each theme
  # walks by time. Keep is the prominent action; Archive and the Priority pin are
  # the deliberate ones. A pinned card wears a "Priority" pill; an AI-suggested one
  # wears a confirmable "Suggested" cue.
  class SkimCardsDemo < Campbooks::Base
    CARDS = [
      {
        category: :notifications, title: "47 CircleCI builds", count: 47, position: 2, total: 8,
        bucket_label: "Today",
        summary: "All resolved — every branch already merged. Nothing here needs you.",
        samples: [ "Workflow failed: connect-backend", "Workflow failed: august", "Workflow canceled: yale-home" ]
      },
      {
        category: :important, title: "Banco BPI — verification code", count: 1, position: 1, total: 8,
        bucket_label: "Today", priority_suggested: true,
        summary: "A one-time code to confirm your transfer. Expires in 10 minutes.",
        samples: []
      },
      {
        category: :personal, title: "3 messages from Jamie", count: 3, position: 3, total: 8,
        bucket_label: "Yesterday", pinned: true,
        summary: "She is asking about lunch tomorrow and shared a Figma file to review.",
        samples: [ "Lunch tomorrow?", "Carte de visite Jamie", "Reminder: edit the file" ]
      }
    ].freeze

    # The same demo content grouped into THEME rings for the Stories viewer
    # (Campbooks::SkimStack): a leading Priority lane, then People and Alerts.
    # Within a ring the clusters are time-ordered (Today → Earlier).
    #
    # A ring's `count` is its number of skim STEPS (clusters), so it always equals
    # `stacks` — a single cluster of 47 emails is one stack to skim, badge "1".
    # (Cluster `count` is separate: it's the emails inside that one card, e.g. 47.)
    RINGS = [
      {
        theme: :priority, label: "Priority", count: 1, stacks: 1, senders: [ "Jamie" ],
        clusters: [
          { category: :personal, title: "Jamie — Figma review", count: 1, pinned: true,
            bucket_label: "Yesterday",
            summary: "You pinned this to deal with it.", samples: [], emails: [], email_ids: [ 201 ],
            position: 1, total: 1 }
        ]
      },
      {
        theme: :personal, label: "People", count: 1, stacks: 1, senders: [ "Jamie" ],
        clusters: [
          { category: :personal, title: "3 messages from Jamie", count: 3, bucket_label: "Today",
            summary: "She is asking about lunch tomorrow and shared a Figma file to review.",
            samples: [ "Lunch tomorrow?", "Carte de visite Jamie", "Reminder: edit the file" ],
            emails: [
              { id: 201, sender: "Jamie", subject: "Lunch tomorrow?", snippet: "Are you free around 1pm? I found a new spot near the studio." },
              { id: 202, sender: "Jamie", subject: "Carte de visite Jamie", snippet: "Sharing my new card design — let me know what you think." },
              { id: 203, sender: "Jamie", subject: "Reminder: edit the file", snippet: "Pushed the latest frames to Figma; your section is the last one." }
            ],
            email_ids: [ 201, 202, 203 ], position: 1, total: 1 }
        ]
      },
      {
        theme: :important, label: "Alerts", count: 2, stacks: 2, senders: [ "Banco BPI", "Google" ],
        clusters: [
          { category: :important, title: "Banco BPI — verification code", count: 1, bucket_label: "Today",
            priority_suggested: true,
            summary: "A one-time code to confirm your transfer. Expires in 10 minutes.",
            samples: [], emails: [], email_ids: [ 101 ], position: 1, total: 2 },
          { category: :important, title: "New login attempt", count: 1, bucket_label: "Yesterday",
            summary: "A sign-in from a new device. Confirm it was you.",
            samples: [], emails: [], email_ids: [ 102 ], position: 2, total: 2 }
        ]
      },
      {
        theme: :notifications, label: "Notifications", count: 1, stacks: 1, senders: [ "CircleCI" ],
        clusters: [
          { category: :notifications, title: "47 CircleCI builds", count: 47, bucket_label: "Today",
            summary: "All resolved — every branch already merged. Nothing here needs you.",
            samples: [ "Workflow failed: connect-backend", "Workflow failed: august", "Workflow canceled: yale-home" ],
            emails: [], email_ids: (300..346).to_a, position: 1, total: 1 }
        ]
      }
    ].freeze

    def view_template
      div(class: "flex flex-wrap items-start gap-4") do
        CARDS.each { |card| render Campbooks::SkimCard.new(**card) }
      end
    end
  end
end
