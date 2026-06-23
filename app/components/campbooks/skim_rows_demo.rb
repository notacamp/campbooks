# frozen_string_literal: true

module Campbooks
  # Preview-only demo of the skimmable inbox treatment: noise recedes, the rows
  # that need a human (personal / important) surface via the chip colour.
  class SkimRowsDemo < Campbooks::Base
    ROWS = [
      { category: :important, sender: "Banco BPI", subject: "Verification code", preview: "One-time code to confirm your transfer — expires in 10 minutes.", time: "09:14", unread: true },
      { category: :personal, sender: "Jamie", subject: "Lunch tomorrow?", preview: "Are you free around 1pm near the studio? Would love to catch up.", time: "08:02", unread: true },
      { category: :notifications, sender: "CircleCI", subject: "Workflow failed: connect-backend", preview: "GRY-308-grant-yale-app-ownership · 2 jobs failed", time: "Tue" },
      { category: :promotions, sender: "Font Awesome", subject: "2 Days Left! 15% off everything", preview: "Your last chance to back the Build Awesome Kickstarter.", time: "Tue" },
      { category: :updates, sender: "Paack", subject: "Your order is on its way", preview: "Encomenda a caminho — estimated delivery tomorrow before noon.", time: "Mon" },
      { category: :social, sender: "LinkedIn", subject: "5 people viewed your profile", preview: "See who has been looking at your profile this week.", time: "Mon" }
    ].freeze

    def view_template
      div(class: "max-w-2xl rounded-lg border border-border overflow-hidden bg-card") do
        ROWS.each { |row| render Campbooks::SkimRow.new(**row) }
      end
    end
  end
end
