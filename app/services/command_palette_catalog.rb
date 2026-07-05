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
      cmd("inbox", I18n.t("command_palette.commands.inbox"), I18n.t("command_palette.categories.navigate"), "mail", root_path),
      cmd("scout", I18n.t("command_palette.commands.scout_ai_chat"), I18n.t("command_palette.categories.navigate"), "sparkles", scout_path),
      cmd("files", I18n.t("command_palette.commands.files"), I18n.t("command_palette.categories.navigate"), "folder", files_path),
      # Workflows is gated off by default until it's production-ready (Features.workflows?).
      *(Features.workflows? ? [ cmd("workflows", I18n.t("command_palette.commands.workflows"), I18n.t("command_palette.categories.navigate"), "workflow", workflows_path) ] : []),
      cmd("email-scans", I18n.t("command_palette.commands.email_scans"), I18n.t("command_palette.categories.navigate"), "search", email_messages_path(inbox_settings: "accounts")),
      cmd("notifications", I18n.t("command_palette.commands.notifications"), I18n.t("command_palette.categories.navigate"), "bell", notifications_path),
      cmd("calendar", I18n.t("command_palette.commands.calendar"), I18n.t("command_palette.categories.navigate"), "calendar", calendar_path),
      cmd("contacts", I18n.t("command_palette.commands.settings_contacts"), I18n.t("command_palette.categories.navigate"), "users", contacts_path)
    ]
  end

  def settings
    [
      cmd("settings", I18n.t("command_palette.commands.settings"), I18n.t("command_palette.categories.settings"), "cog", settings_root_path),
      cmd("settings-ai", I18n.t("command_palette.commands.settings_ai"), I18n.t("command_palette.categories.settings"), "sparkles", settings_ai_path),
      cmd("settings-account", I18n.t("command_palette.commands.settings_account"), I18n.t("command_palette.categories.settings"), "users", settings_account_path),
      cmd("settings-tags", I18n.t("command_palette.commands.settings_tags"), I18n.t("command_palette.categories.settings"), "tag", email_messages_path(inbox_settings: "tags")),
      cmd("settings-doctypes", I18n.t("command_palette.commands.settings_doctypes"), I18n.t("command_palette.categories.docs"), "file-text", email_messages_path(inbox_settings: "document_types")),
      cmd("settings-signatures", I18n.t("command_palette.commands.settings_signatures"), I18n.t("command_palette.categories.settings"), "pen", email_messages_path(inbox_settings: "signatures")),
      cmd("settings-members", I18n.t("command_palette.commands.settings_members"), I18n.t("command_palette.categories.settings"), "users", settings_members_path),
      cmd("settings-notifications", I18n.t("command_palette.commands.settings_notifications"), I18n.t("command_palette.categories.settings"), "bell", settings_notifications_path),
      cmd("settings-sync", I18n.t("command_palette.commands.settings_sync"), I18n.t("command_palette.categories.settings"), "at-sign", email_messages_path(inbox_settings: "accounts")),
      cmd("settings-integrations", I18n.t("command_palette.commands.settings_integrations"), I18n.t("command_palette.categories.settings"), "cog", settings_integrations_root_path),
      cmd("settings-notion", I18n.t("command_palette.commands.settings_notion"), I18n.t("command_palette.categories.settings"), "cog", settings_integrations_notion_path),
      cmd("settings-gdrive", I18n.t("command_palette.commands.settings_gdrive"), I18n.t("command_palette.categories.settings"), "folder", settings_integrations_google_drive_path),
      cmd("settings-zdrive", I18n.t("command_palette.commands.settings_zdrive"), I18n.t("command_palette.categories.settings"), "folder", settings_integrations_zoho_drive_path)
    ]
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
  # are added client-side in command_palette_controller (they depend on the view+date
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
