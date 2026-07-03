# frozen_string_literal: true

module Campbooks
  module Calendar
    # The /calendar page's calendar-management pane (Google-Calendar-style):
    # calendars grouped by the email account that owns them, with a per-user
    # show/hide checkbox per calendar (CalendarVisibilitiesController — display
    # only, personal), and manager-gated affordances riding the existing
    # account-wide endpoints: recolor / stop syncing (CalendarsController#update),
    # enable a discovered-but-off calendar, and an on-demand provider list
    # refresh (CalendarAccountsController#refresh).
    #
    # Rendered twice by calendar/index: inside the desktop <aside> and inside
    # the mobile <dialog> — it must stay free of element ids for that reason.
    # Forms auto-submit color changes via the page's `calendar-sidebar`
    # controller (change->calendar-sidebar#submit).
    #
    # @param accounts [Enumerable<CalendarAccount>] readable accounts, calendars preloaded
    # @param user [User] viewer (hidden-calendar state lives on the user row)
    # @param view [String] current calendar view, round-tripped through toggles
    # @param date [Date] current anchor date, round-tripped through toggles
    # @param managed_account_ids [Array] ids of accounts the viewer can manage
    class Sidebar < Campbooks::Base
      CHECK_SVG = %(<svg viewBox="0 0 24 24" class="h-3 w-3" fill="none" stroke="%{stroke}" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6L9 17l-5-5"/></svg>)
      CHEVRON_SVG = %(<svg viewBox="0 0 24 24" class="h-3.5 w-3.5 flex-shrink-0 transition-transform group-open:rotate-90" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 5l7 7-7 7"/></svg>)
      ELLIPSIS_SVG = %(<svg viewBox="0 0 24 24" class="h-4 w-4" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="19" cy="12" r="1.7"/></svg>)
      PLUS_SVG = %(<svg viewBox="0 0 24 24" class="h-3.5 w-3.5 flex-shrink-0" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>)
      REFRESH_SVG = %(<svg viewBox="0 0 24 24" class="h-3.5 w-3.5 flex-shrink-0" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182m0-4.991v4.99"/></svg>)
      UPLOAD_SVG = %(<svg viewBox="0 0 24 24" class="h-3.5 w-3.5 flex-shrink-0" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5m-13.5-9L12 3m0 0 4.5 4.5M12 3v13.5"/></svg>)

      def initialize(accounts:, user:, view:, date:, managed_account_ids: [])
        @accounts = accounts
        @user = user
        @view = view
        @date = date
        @managed_account_ids = managed_account_ids
      end

      def view_template
        div(class: "flex flex-col gap-4") do
          @accounts.each { |account| account_section(account) }
          footer_actions
        end
      end

      private

      def account_section(account)
        calendars = account.calendars.sort_by { |c| [ c.is_primary ? 0 : 1, c.name.to_s.downcase ] }
        synced, unsynced = calendars.partition(&:syncing?)
        manager = @managed_account_ids.include?(account.id)

        details(open: true, class: "group") do
          summary(class: "flex cursor-pointer list-none items-center gap-1.5 rounded-md px-1.5 py-1 hover:bg-muted [&::-webkit-details-marker]:hidden") do
            raw safe(CHEVRON_SVG)
            span(class: "min-w-0 flex-1 truncate text-[11px] font-semibold uppercase tracking-wide text-muted-foreground",
                 title: account.email_address) { account.display_name }
          end
          div(class: "mt-1 space-y-0.5") do
            if synced.any?
              synced.each { |cal| calendar_row(account, cal, manager: manager) }
            else
              p(class: "px-1.5 py-1 text-xs text-muted-foreground") { t(".none_synced") }
            end
            add_calendars(account, unsynced, manager: manager) if unsynced.any?
            refresh_row(account) if manager
          end
        end
      end

      # ── Synced calendar row: visibility checkbox + manager menu ──────────────

      def calendar_row(account, cal, manager:)
        div(class: "flex items-center gap-0.5") do
          visibility_form(cal)
          calendar_menu(account, cal) if manager
        end
      end

      # The whole row-left is one submit button (GCal-style: click anywhere on the
      # name to show/hide). Explicit hidden=0|1 keeps a double submit idempotent.
      def visibility_form(cal)
        hidden = @user.calendar_hidden?(cal)
        color = cal.display_color
        form(action: helpers.calendar_visibility_path(cal), method: "post", accept_charset: "UTF-8", class: "min-w-0 flex-1") do
          input(type: "hidden", name: "_method", value: "patch")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
          input(type: "hidden", name: "hidden", value: hidden ? "0" : "1")
          input(type: "hidden", name: "view", value: @view)
          input(type: "hidden", name: "date", value: @date.iso8601)
          button(type: "submit", role: "checkbox", aria_checked: (!hidden).to_s,
                 title: hidden ? t(".show_calendar") : t(".hide_calendar"),
                 class: "flex w-full cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 text-left transition-colors hover:bg-muted") do
            checkbox_square(color, checked: !hidden)
            span(class: "min-w-0 flex-1 truncate text-[13px] #{hidden ? 'text-muted-foreground' : 'text-foreground'}") { cal.name }
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground") { t(".primary") } if cal.is_primary
          end
        end
      end

      def checkbox_square(color, checked:)
        if checked
          span(class: "flex h-4 w-4 flex-shrink-0 items-center justify-center rounded-[4px]", style: "background-color: #{color}") do
            raw safe(CHECK_SVG % { stroke: contrast_on(color) })
          end
        else
          span(class: "h-4 w-4 flex-shrink-0 rounded-[4px] border-2", style: "border-color: #{color}")
        end
      end

      # Manager-only "⋯" popover: calendar color (auto-submits on pick) and the
      # account-wide stop-syncing action. Native <details>; the page controller
      # closes any open one on outside click.
      def calendar_menu(account, cal)
        details(class: "relative flex-shrink-0", data: { calendar_sidebar_target: "menu" }) do
          summary(class: "flex h-6 w-6 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground [&::-webkit-details-marker]:hidden",
                  title: t(".calendar_options"), aria_label: t(".calendar_options")) do
            raw safe(ELLIPSIS_SVG)
          end
          div(class: "absolute right-0 z-20 mt-1 w-64 rounded-lg border border-border bg-card p-3 shadow-lg") do
            p(class: "mb-1.5 text-xs font-medium text-muted-foreground") { t(".color_label") }
            form(action: helpers.calendar_account_calendar_path(account, cal), method: "post", accept_charset: "UTF-8",
                 data: { action: "change->calendar-sidebar#submit" }) do
              input(type: "hidden", name: "_method", value: "patch")
              input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
              # selected is the raw column: blank means "inherit the account color".
              render Campbooks::ColorSwatchPicker.new(name: "calendar[color]", selected: cal.color, none_label: t(".account_color"))
            end
            div(class: "mt-3 border-t border-border pt-2") do
              form(action: helpers.calendar_account_calendar_path(account, cal), method: "post", accept_charset: "UTF-8") do
                input(type: "hidden", name: "_method", value: "patch")
                input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
                input(type: "hidden", name: "calendar[syncing]", value: "0")
                button(type: "submit",
                       data: { turbo_confirm: t(".stop_syncing_confirm", name: cal.name) },
                       class: "w-full cursor-pointer rounded-md px-1.5 py-1 text-left text-[13px] text-red-600 transition-colors hover:bg-red-50 dark:hover:bg-red-500/10") do
                  t(".stop_syncing")
                end
              end
            end
          end
        end
      end

      # ── Discovered-but-off calendars: the "import more" disclosure ───────────

      def add_calendars(account, unsynced, manager:)
        details(class: "mt-1") do
          summary(class: "flex cursor-pointer list-none items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground [&::-webkit-details-marker]:hidden") do
            raw safe(PLUS_SVG)
            span { t(".add_calendars", count: unsynced.size) }
          end
          div(class: "mt-0.5 space-y-0.5") do
            p(class: "px-1.5 py-1 text-[11px] text-muted-foreground") { t(".managers_only") } unless manager
            unsynced.each do |cal|
              div(class: "flex items-center gap-2 rounded-md px-1.5 py-1") do
                # Dashed square = discovered at the provider but not syncing yet.
                span(class: "h-4 w-4 flex-shrink-0 rounded-[4px] border-2 border-dashed", style: "border-color: #{cal.display_color}")
                span(class: "min-w-0 flex-1 truncate text-[13px] text-muted-foreground") { cal.name }
                if manager
                  form(action: helpers.calendar_account_calendar_path(account, cal), method: "post", accept_charset: "UTF-8", class: "flex-shrink-0") do
                    input(type: "hidden", name: "_method", value: "patch")
                    input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
                    input(type: "hidden", name: "calendar[syncing]", value: "1")
                    button(type: "submit", class: "cursor-pointer rounded-md px-2 py-0.5 text-xs font-medium text-accent-700 transition-colors hover:bg-accent-50 dark:text-accent-300 dark:hover:bg-accent-500/10") { t(".add") }
                  end
                end
              end
            end
          end
        end
      end

      # Re-pull the provider's calendar list (async; responds with a toast).
      def refresh_row(account)
        form(action: helpers.refresh_calendar_account_path(account), method: "post", accept_charset: "UTF-8", class: "mt-0.5") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token, autocomplete: "off")
          button(type: "submit", class: "flex w-full cursor-pointer items-center gap-1.5 rounded-md px-1.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground") do
            raw safe(REFRESH_SVG)
            span { t(".refresh_list") }
          end
        end
      end

      def footer_actions
        div(class: "space-y-0.5 border-t border-border pt-3") do
          a(href: helpers.new_calendar_import_path,
            class: "flex items-center gap-1.5 rounded-md px-1.5 py-1 text-[13px] text-muted-foreground no-underline transition-colors hover:bg-muted hover:text-foreground") do
            raw safe(UPLOAD_SVG)
            span { t(".import_ics") }
          end
          a(href: helpers.settings_integrations_calendars_path,
            class: "flex items-center gap-1.5 rounded-md px-1.5 py-1 text-[13px] text-muted-foreground no-underline transition-colors hover:bg-muted hover:text-foreground") do
            raw safe(PLUS_SVG)
            span { t(".connect_account") }
          end
        end
      end
    end
  end
end
