# frozen_string_literal: true

module Campbooks
  # Presentation for Skim theme rings (the tray menu + the viewer header): the
  # icon and hue for each theme. Single source of truth shared by SkimRing (tray)
  # and SkimStack (viewer), reusing the CategoryChip icons so a theme reads the
  # same everywhere. Labels come from Emails::SkimBuilder (carried on each ring).
  module SkimTheme
    # Recency/affect hue per theme (oklch hue angle). Priority = gold; People =
    # accent violet; Alerts = amber; the rest spread across calm hues.
    HUE = {
      follow_ups:    70,
      starred:       40,
      priority:      45,
      pending:       25,
      personal:      276,
      important:     70,
      updates:       200,
      notifications: 250,
      social:        330,
      promotions:    300,
      unknown:       230
    }.freeze
    DEFAULT_HUE = 276

    # Bookmark (Priority) and lightning ("Skim all"); themes reuse CategoryChip.
    PIN = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"/>'
    ALL = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/>'
    # Star (starred senders) and clock (pending senders awaiting a decision).
    STAR = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.48 3.5a.56.56 0 011.04 0l2.12 5.11a.56.56 0 00.48.35l5.52.44c.5.04.7.66.32.99l-4.2 3.6a.56.56 0 00-.18.56l1.28 5.38a.56.56 0 01-.84.61l-4.72-2.88a.56.56 0 00-.59 0l-4.72 2.88a.56.56 0 01-.84-.61l1.28-5.38a.56.56 0 00-.18-.56l-4.2-3.6a.56.56 0 01.32-.99l5.52-.44a.56.56 0 00.48-.35z"/>'
    PENDING = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z"/>'
    # Follow-ups: a clock (you're waiting on a reply).
    FOLLOW_UP = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z"/>'

    module_function

    def hue(theme)
      HUE[theme&.to_sym] || DEFAULT_HUE
    end

    # SVG inner markup for a theme's icon. nil → "Skim all"; :priority → bookmark;
    # :starred → star; :pending → clock; otherwise the matching CategoryChip icon.
    def icon(theme)
      case theme&.to_sym
      when nil then ALL
      when :priority then PIN
      when :starred then STAR
      when :pending then PENDING
      when :follow_ups then FOLLOW_UP
      else Campbooks::CategoryChip::ICONS[theme.to_sym] || ALL
      end
    end
  end
end
