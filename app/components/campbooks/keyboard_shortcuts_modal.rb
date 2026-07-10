# frozen_string_literal: true

module Campbooks
  class KeyboardShortcutsModal < Campbooks::Base
    Shortcut = Struct.new(:key, :label, :context, keyword_init: true)

    def initialize(open: false, **attrs)
      @open = open
      @attrs = attrs
    end

    # Defined as a method so t() resolves at render time (not class-load).
    def shortcuts
      [
        { key: "↓ ↑", label: t(".shortcuts.navigate_threads"), context: t(".contexts.navigation") },
        { key: "g h",   label: t(".shortcuts.nav_home"),         context: t(".contexts.navigation") },
        { key: "g m",   label: t(".shortcuts.nav_mail"),         context: t(".contexts.navigation") },
        { key: "g c",   label: t(".shortcuts.nav_calendar"),     context: t(".contexts.navigation") },
        { key: "g s",   label: t(".shortcuts.nav_scout"),        context: t(".contexts.navigation") },
        { key: "g f",   label: t(".shortcuts.nav_files"),        context: t(".contexts.navigation") },
        { key: "g p",   label: t(".shortcuts.nav_contacts"),     context: t(".contexts.navigation") },
        { key: "g o",   label: t(".shortcuts.nav_organizations"), context: t(".contexts.navigation") },
        { key: "g a",   label: t(".shortcuts.nav_activity"),     context: t(".contexts.navigation") },
        { key: "↑↓ / j k", label: t(".shortcuts.feed_navigate"), context: t(".contexts.feed") },
        { key: "→ / ⏎",    label: t(".shortcuts.feed_primary"),  context: t(".contexts.feed") },
        { key: "←",         label: t(".shortcuts.feed_dismiss"),  context: t(".contexts.feed") },
        { key: "e r c",     label: t(".shortcuts.feed_actions"),  context: t(".contexts.feed") },
        { key: "e",       label: t(".shortcuts.archive"),          context: t(".contexts.email") },
        { key: "#",       label: t(".shortcuts.delete"),           context: t(".contexts.email") },
        { key: "r",       label: t(".shortcuts.reply"),            context: t(".contexts.email") },
        { key: "a",       label: t(".shortcuts.reply_all"),        context: t(".contexts.email") },
        { key: "f",       label: t(".shortcuts.forward"),          context: t(".contexts.email") },
        { key: "x",       label: t(".shortcuts.toggle_selection"), context: t(".contexts.selection") },
        { key: "Shift + I", label: t(".shortcuts.mark_read"),      context: t(".contexts.selection") },
        { key: "Shift + U", label: t(".shortcuts.mark_unread"),    context: t(".contexts.selection") },
        { key: "Esc",     label: t(".shortcuts.clear_selection"),  context: t(".contexts.selection") },
        { key: "c",       label: t(".shortcuts.compose"),          context: t(".contexts.general") },
        { key: "?",       label: t(".shortcuts.show_shortcuts"),   context: t(".contexts.general") },
        { key: "⌘K",     label: t(".shortcuts.command_palette"),  context: t(".contexts.general") },
        { key: "t",       label: t(".shortcuts.cal_today"),        context: t(".contexts.calendar") },
        { key: "j / →",   label: t(".shortcuts.cal_next"),         context: t(".contexts.calendar") },
        { key: "k / ←",   label: t(".shortcuts.cal_prev"),         context: t(".contexts.calendar") },
        { key: "d w m a", label: t(".shortcuts.cal_views"),        context: t(".contexts.calendar") },
        { key: "c",       label: t(".shortcuts.cal_new"),          context: t(".contexts.calendar") },
        { key: "↑↓ / j k", label: t(".shortcuts.reminders_navigate"), context: t(".contexts.reminders") },
        { key: "⏎",       label: t(".shortcuts.reminders_confirm"),   context: t(".contexts.reminders") },
        { key: "s",       label: t(".shortcuts.reminders_snooze"),    context: t(".contexts.reminders") },
        { key: "d",       label: t(".shortcuts.reminders_dismiss"),   context: t(".contexts.reminders") }
      ]
    end

    def view_template
      dialog(
        id: "keyboard-shortcuts-modal",
        class: "rounded-xl shadow-2xl border border-gray-200 p-0 overflow-hidden backdrop:bg-black/30 m-auto",
        aria: { label: t(".title") },
        open: @open ? "" : nil,
        **@attrs
      ) do
        div(class: "w-[calc(100vw-2rem)] max-w-[480px]") do
          # Header
          div(class: "px-5 py-4 border-b border-gray-200 flex items-center justify-between") do
            h2(class: "text-sm font-semibold text-gray-900") { t(".title") }
            form(method: "dialog") do
              button(
                type: "submit",
                class: "p-1 rounded hover:bg-gray-100 text-gray-400 hover:text-gray-600 transition-colors cursor-pointer border-0 bg-transparent",
                aria_label: t("shared.actions.close")
              ) do
                raw(safe('<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'))
              end
            end
          end

          # Body — grouped by context
          div(class: "px-5 py-4 space-y-5 max-h-[60vh] overflow-y-auto") do
            grouped = shortcuts.group_by { |s| s[:context] }
            grouped.each do |context, shortcuts|
              div do
                h3(class: "text-[10px] font-semibold text-gray-400 uppercase tracking-wider mb-2") { context }
                div(class: "space-y-0") do
                  shortcuts.each do |s|
                    div(class: "flex items-center justify-between py-1.5") do
                      kbd(class: "inline-flex items-center h-5 px-1.5 rounded bg-gray-100 border border-gray-200 text-[11px] font-mono font-medium text-gray-700 min-w-[28px] justify-center") do
                        raw(safe(s[:key]))
                      end
                      span(class: "text-[12px] text-gray-600") { s[:label] }
                    end
                  end
                end
              end
            end
          end

          # Footer
          div(class: "px-4 py-2 border-t border-gray-100 text-[10px] text-gray-400") do
            t(".footer_hint")
          end
        end
      end
    end
  end
end
