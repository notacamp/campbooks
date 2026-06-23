# frozen_string_literal: true

module Campbooks
  # Hover card for one of *our own* linked mailboxes — the account-level analogue
  # of ContactPopover. Surfaced from the inbox account-filter avatars so a glance
  # answers "is this mailbox connected, and is it actually syncing?" without a trip
  # to settings. Lazily fetched via the shared `contact-popover` Stimulus
  # controller (see EmailAccountsController#popover).
  #
  # Kept query-free: the message count is passed in so the component stays a pure
  # presenter (and renders in Lookbook from a plain EmailAccount.new).
  class EmailAccountPopover < Campbooks::Base
    # @param account [EmailAccount] the linked mailbox to describe
    # @param message_count [Integer] ingested messages for this account
    def initialize(account:, message_count: 0, **attrs)
      @account = account
      @message_count = message_count
      @attrs = attrs
    end

    def view_template
      div(class: "bg-card rounded-xl shadow-lg border border-gray-200 p-4 w-72", **@attrs) do
        header
        status_row
        stats_row
        cta
      end
    end

    private

    def header
      div(class: "flex items-center gap-2.5 mb-3") do
        div(
          class: "w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 text-xs font-semibold text-white",
          style: "background-color: #{@account.color}"
        ) { plain(@account.avatar_initial) }
        div(class: "min-w-0") do
          div(class: "text-sm font-semibold text-gray-900 truncate") { plain(@account.display_name) }
          div(class: "text-[11px] text-gray-400 truncate") { plain(@account.email_address) }
        end
      end
    end

    # Connection pill + live sync state, side by side — the heart of the card.
    def status_row
      div(class: "flex items-center gap-2 mb-3") do
        connection_badge
        sync_state
      end
    end

    def connection_badge
      if @account.active?
        badge("bg-green-100 text-green-700 dark:bg-green-500/15 dark:text-green-300", t(".connected"))
      else
        badge("bg-red-100 text-red-700 dark:bg-red-500/15 dark:text-red-300", t(".disconnected"))
      end
    end

    def badge(css, text)
      span(class: "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium #{css}") { plain(text) }
    end

    def sync_state
      if @account.actively_scanning?
        div(class: "flex items-center gap-1.5 text-[11px] text-accent-600") do
          render(Campbooks::Spinner.new(size: :sm))
          span { t(".syncing") }
        end
      elsif @account.last_scanned_at
        span(class: "text-[11px] text-gray-500", title: l(@account.last_scanned_at, format: :full)) do
          plain(t(".synced_ago", time: helpers.time_ago_in_words(@account.last_scanned_at)))
        end
      else
        span(class: "text-[11px] text-amber-600 dark:text-amber-400") { t(".never_synced") }
      end
    end

    def stats_row
      div(class: "flex items-center gap-4 mb-3 text-[11px] text-gray-400") do
        div(class: "flex items-center gap-1") do
          span(class: "text-xs font-semibold text-gray-700") { plain(@message_count.to_s) }
          span { t(".emails_label") }
        end
        span { plain(helpers.human_enum(EmailAccount, :provider, @account.provider)) }
      end
    end

    def cta
      a(
        href: "/email_messages?inbox_settings=accounts",
        class: "inline-flex items-center gap-1 text-xs font-medium text-accent-600 hover:text-accent-700",
        data: { turbo_frame: "_top" }
      ) { plain(t(".manage")) }
    end
  end
end
