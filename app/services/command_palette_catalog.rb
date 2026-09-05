# frozen_string_literal: true

# Static command catalog for the Cmd+K palette: navigation, settings pages, and
# global actions. Built with real route helpers (no hardcoded paths) and gated by
# the user's role. Serialized to JSON and handed to the Stimulus controller, which
# filters it client-side for instant, offline results. Per-message email actions
# (reply, archive, move-to-folder) stay in the controller — they depend on runtime
# context values, not this static list.
class CommandPaletteCatalog
  include Rails.application.routes.url_helpers

  def self.for(user)
    new(user).commands
  end

  def initialize(user)
    @user = user
  end

  def commands
    navigate + settings + actions + calendar_commands + admin
  end

  private

  def cmd(id, name, category, icon, url, method: "get")
    { id: id, name: name, category: category, icon: icon, url: url, method: method }
  end

  def navigate
    [
      cmd("now", I18n.t("command_palette.commands.now"), I18n.t("command_palette.categories.navigate"), "grid", now_path),
      cmd("people", I18n.t("command_palette.commands.people"), I18n.t("command_palette.categories.navigate"), "users", people_path),
      cmd("paper", I18n.t("command_palette.commands.paper"), I18n.t("command_palette.categories.navigate"), "file", paper_path),
      # Money (the obligations surface) lives wherever the accounting module does.
      *(Features.accounting? ? [ cmd("money", I18n.t("command_palette.commands.money"), I18n.t("command_palette.categories.navigate"), "credit-card", money_path) ] : []),
      cmd("time", I18n.t("command_palette.commands.time"), I18n.t("command_palette.categories.navigate"), "calendar", time_path),
      cmd("scout", I18n.t("command_palette.commands.scout_ai_chat"), I18n.t("command_palette.categories.navigate"), "sparkles", scout_path),
      cmd("files", I18n.t("command_palette.commands.files"), I18n.t("command_palette.categories.navigate"), "folder", files_path),
      # Workflows is gated off by default until it's production-ready (Features.workflows?).
      *(Features.workflows? ? [ cmd("workflows", I18n.t("command_palette.commands.workflows"), I18n.t("command_palette.categories.navigate"), "workflow", workflows_path) ] : []),
      # Bank statements — the accounting reconciliation list, reached from Money. Gated by Features.accounting?.
      *(Features.accounting? ? [ cmd("statements", I18n.t("command_palette.commands.statements"), I18n.t("command_palette.categories.navigate"), "credit-card", money_statements_path) ] : []),
      cmd("email-scans", I18n.t("command_palette.commands.email_scans"), I18n.t("command_palette.categories.navigate"), "search", settings_inbox_section_path("accounts")),
      cmd("notifications", I18n.t("command_palette.commands.notifications"), I18n.t("command_palette.categories.navigate"), "bell", notifications_path),
      cmd("calendar", I18n.t("command_palette.commands.calendar"), I18n.t("command_palette.categories.navigate"), "calendar", calendar_path),
      cmd("contacts", I18n.t("command_palette.commands.settings_contacts"), I18n.t("command_palette.categories.navigate"), "users", contacts_path)
    ]
  end

  def settings
    context = Settings::Catalog::Context.new(
      user:   @user,
      native: false
    )
    cat = I18n.t("command_palette.categories.settings")
    # Map catalog icon symbols to palette icon vocabulary
    icon_map = {
      user: "users", lock: "key", bell: "bell", spark: "sparkles", cpu: "sparkles",
      chat: "sparkles", template: "file-text", "mail-template": "mail", zap: "workflow",
      tag: "tag", layers: "grid", filter: "filter", pen: "pen", eye: "eye",
      file: "file-text", branch: "git-branch", at: "at-sign", calendar: "calendar",
      drive: "folder", notion: "cog", cloud: "cloud", plug: "plug", key: "key",
      building: "cog", users: "users", grid: "grid", "shield-check": "shield",
      card: "credit-card", pulse: "cog", sliders: "cog"
    }
    Settings::Catalog.groups(context).flat_map do |group|
      group.items.map do |item|
        icon = icon_map[item[:icon]] || "cog"
        cmd("settings-#{item[:key]}", item[:label], cat, icon, item[:path])
      end
    end
  end

  def actions
    [
      # Honors the user's compose_default: Desk navigates, Dock POSTs a sheet.
      (if Current.user&.composes_in_dock?
         cmd("new-email", I18n.t("command_palette.commands.new_email"), I18n.t("command_palette.categories.actions"), "pen", compose_new_email_messages_path(mode: "new_message"), method: "post")
       else
         cmd("new-email", I18n.t("command_palette.commands.new_email"), I18n.t("command_palette.categories.actions"), "pen", new_email_message_path)
       end),
      # Workflows is gated off by default until it's production-ready (Features.workflows?).
      *(Features.workflows? ? [ cmd("new-workflow", I18n.t("command_palette.commands.new_workflow"), I18n.t("command_palette.categories.actions"), "plus", new_workflow_path) ] : []),
      cmd("new-calendar-event", I18n.t("command_palette.commands.new_calendar_event"), I18n.t("command_palette.categories.actions"), "calendar", calendar_path(new_event: "1")),
      cmd("scan-emails", I18n.t("command_palette.commands.scan_emails"), I18n.t("command_palette.categories.actions"), "search", inbox_settings_accounts_scan_path, method: "post")
    ]
  end

  # Static calendar destinations (view switches + today). Page-relative previous/next
  # are added client-side in scout_overlay_controller (they depend on the view+date
  # currently rendered).
  def calendar_commands
    cat = I18n.t("command_palette.categories.calendar")
    [
      cmd("calendar-today", I18n.t("command_palette.commands.calendar_today"), cat, "calendar", calendar_path(date: Date.current.iso8601)),
      cmd("calendar-agenda", I18n.t("command_palette.commands.calendar_agenda"), cat, "calendar", calendar_path(view: "agenda")),
      cmd("calendar-day", I18n.t("command_palette.commands.calendar_day"), cat, "calendar", calendar_path(view: "day")),
      cmd("calendar-week", I18n.t("command_palette.commands.calendar_week"), cat, "calendar", calendar_path(view: "week")),
      cmd("calendar-month", I18n.t("command_palette.commands.calendar_month"), cat, "calendar", calendar_path(view: "month"))
    ]
  end

  def admin
    return [] unless @user&.app_admin?

    [
      cmd("admin", I18n.t("command_palette.commands.admin_dashboard"), I18n.t("command_palette.categories.admin"), "grid", admin_root_path),
      cmd("admin-signups", I18n.t("command_palette.commands.admin_signups"), I18n.t("command_palette.categories.admin"), "users", admin_signup_requests_path),
      cmd("admin-invitations", I18n.t("command_palette.commands.admin_invitations"), I18n.t("command_palette.categories.admin"), "mail", admin_invitations_path),
      cmd("admin-users", I18n.t("command_palette.commands.admin_users"), I18n.t("command_palette.categories.admin"), "users", admin_users_path)
    ]
  end
end
