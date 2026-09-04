module NavigationHelper
  # Primary areas that expose sub-sections through the contextual section nav
  # (the thin bar under the topbar). Each area is an ordered list of tabs;
  # a tab maps a controller to a label and an index path. The bar renders
  # whenever the current controller belongs to one of these areas.
  # Document-type management moved into the inbox settings modal, so the Docs
  # area no longer needs a contextual tab bar. Kept as an extension point.
  SECTION_AREAS = [].freeze

  # Gmail-style two-step keyboard shortcut keys for the primary nav destinations.
  # Press `g` to arm navigation mode, then the key below to navigate. Both
  # Campbooks::NavRail and Campbooks::BottomNav read this map to render their
  # key-badge chips and aria-keyshortcuts attributes.
  NAV_SHORTCUT_KEYS = {
    now:    "n",
    people: "p",
    paper:  "d",
    money:  "m",
    time:   "t"
  }.freeze

  # Inline SVG bodies for the primary nav icons (rendered raw, mirroring the
  # Campbooks::Logo component's approach). Scout is a filled spark; the rest are
  # stroked line icons sharing one visual weight.
  NAV_ICON_PATHS = {
    now:    '<path d="m12 3 9 4.5-9 4.5-9-4.5z"/><path d="m3 12 9 4.5 9-4.5"/><path d="m3 16.5 9 4.5 9-4.5"/>',
    people: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
    paper:  '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>',
    money:  '<rect x="2" y="6" width="20" height="12" rx="2"/><circle cx="12" cy="12" r="2.5"/><path d="M6 12h.01M18 12h.01"/>',
    time:   '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>'
  }.freeze
  # Four-point spark, centered in the 24×24 box (tips at 12,5 · 19.5,12 · 12,19 ·
  # 4.5,12 → center 12,12) so it sits dead-center inside the Ember tile.
  NAV_SCOUT_SPARK = '<path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/>'

  # Block-level nav link for the mobile slide-down menu. Full-width tap target,
  # mirrors the active-state logic of the desktop `nav_link` but styled for a
  # stacked list. Uses theme tokens so it reads correctly in dark mode.
  def mobile_nav_link(text, path, also_active_for: [])
    active = ([ path ] + Array(also_active_for)).any? { |p| request.path.start_with?(p) }
    classes = class_names(
      "flex items-center px-3 py-2.5 rounded-lg text-[15px] font-medium transition-colors",
      active ? "bg-accent-50 text-accent-700" : "text-foreground hover:bg-muted"
    )
    link_to text, path, class: classes, data: { action: "click->mobile-menu#close" }
  end

  # ── Primary navigation (the Instagram-style rail + bottom bar) ─────────────
  # Single source of truth for the top-level destinations, consumed by BOTH
  # Campbooks::NavRail (desktop left rail) and Campbooks::BottomNav (mobile tab
  # bar) so the two never drift on items, order, active state, or icons. Each
  # item: { key:, label:, path:, ember:, active: }. Admin/Settings are
  # intentionally absent here: they live in the avatar menu, not the primary nav.
  # Five places reframe the app around decisions rather than folders: Now (the
  # decision deck), People (mail + contacts + organizations), Paper (files +
  # documents), Money (accounting), Time (calendar + reminders + tasks). There is
  # no Scout tile — Scout is reached from the docked bar, ⌘K, the rail search,
  # and the `g s` chord. Money rides the same accounting gate as before.
  def primary_nav_items
    [
      nav_item(:now, t("shared.nav.now"), now_path, exact: false, badge: nav_attention.dot?(:home)),
      nav_item(:people, t("shared.nav.people"), people_path,
        also_active_for: [ email_messages_path, contacts_path, organizations_path ],
        badge: nav_attention.dot?(:mail)),
      nav_item(:paper, t("shared.nav.paper"), paper_path,
        also_active_for: [ files_path, documents_path ], badge: nav_attention.dot?(:files)),
      (nav_item(:money, t("shared.nav.money"), money_path,
        also_active_for: [ accounting_path, "/reconciliations" ]) if Features.accounting?),
      nav_item(:time, t("shared.nav.time"), time_path,
        also_active_for: [ calendar_path, calendar_events_path, reminders_path, tasks_path ],
        badge: nav_attention.dot?(:calendar))
    ].compact
  end

  # Memoized per request: the "action required" dots for the primary nav, read by
  # primary_nav_items above and rendered by Campbooks::NavRail / BottomNav.
  def nav_attention
    @nav_attention ||= Navigation::Attention.new(current_user)
  end

  # One nav item with its computed active state. Active matching mirrors the
  # legacy nav_link: exact for Home (every path starts with "/"), prefix for the
  # rest (so /email_messages/123 keeps Mail lit). The :shortcut field carries the
  # single-key letter for the two-step `g <key>` navigation shortcut (nil when
  # the destination has no shortcut assignment).
  def nav_item(key, label, path, ember: false, exact: false, also_active_for: [], badge: false)
    candidates = [ path ] + Array(also_active_for)
    active = exact ? candidates.include?(request.path) : candidates.any? { |p| request.path.start_with?(p) }
    { key: key, label: label, path: path, ember: ember, active: active, badge: badge,
      shortcut: NAV_SHORTCUT_KEYS[key.to_sym] }
  end

  # html_safe inline SVG for a nav icon. Scout is filled; the rest are stroked.
  def nav_icon_svg(key, css_class: "w-[22px] h-[22px]")
    if key.to_sym == :scout
      svg_tag(NAV_SCOUT_SPARK, css_class, fill: true)
    else
      svg_tag(NAV_ICON_PATHS.fetch(key.to_sym), css_class)
    end
  end

  def svg_tag(inner, css_class, fill: false)
    attrs = if fill
      %(viewBox="0 0 24 24" fill="currentColor")
    else
      %(viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round")
    end
    %(<svg class="#{css_class}" #{attrs} aria-hidden="true">#{inner}</svg>).html_safe
  end

  # Whether the given module key is visible for the current workspace.
  # Returns true when the workspace has no template setting, or when the module
  # is explicitly enabled. Used by primary_nav_items to honour template-driven
  # visibility choices without blocking non-templated workspaces.
  def workspace_module_visible?(key)
    return true unless current_user&.workspace
    current_user.workspace.module_visible?(key)
  end

  # Grouped settings navigation, shared by the settings sidebar
  # (app/views/settings/_sidebar.html.erb) and the topbar user menu
  # (app/views/shared/_user_menu.html.erb) so both expose the same sections in
  # the same order. Each group is [heading, items]; each item is
  # [path, active_keys, label, icon_path]. A sidebar link is active when
  # current_section is in its active_keys list (the user menu ignores active_keys
  # since it renders outside the settings controllers).
  # Five places: Scout's memory, Connections, Workspace & people, Account, Plan.
  def settings_nav_groups
    behaviour_sections = InboxSettings::Sections::ALL
      .map { |section| "inbox_#{section[:key]}" }
      .reject { |key| key == "inbox_accounts" }

    items = [
      [ settings_memory_path,
        %w[memory ai ai_prompts pipelines document_templates email_templates] + behaviour_sections,
        t("navigation.settings.memory"),
        "M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.456-2.456L14.25 6l1.035-.259a3.375 3.375 0 002.456-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 00-2.456 2.456z" ],
      [ settings_integrations_root_path,
        %w[integrations notion google_drive zoho_drive calendars inbox_accounts api_clients],
        t("navigation.settings.connections"),
        "M11 4a2 2 0 114 0v1a1 1 0 001 1h3a1 1 0 011 1v3a1 1 0 01-1 1h-1a2 2 0 100 4h1a1 1 0 011 1v3a1 1 0 01-1 1h-3a1 1 0 01-1-1v-1a2 2 0 10-4 0v1a1 1 0 01-1 1H7a1 1 0 01-1-1v-3a1 1 0 00-1-1H4a2 2 0 110-4h1a1 1 0 001-1V7a1 1 0 011-1h3a1 1 0 001-1V4z" ],
      [ settings_root_path,
        %w[general setup_template members data_privacy system_health],
        t("navigation.settings.workspace_people"),
        "M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" ],
      [ settings_account_path,
        %w[account security totp passkeys recovery_codes email_otp audit_log notifications],
        t("navigation.settings.account"),
        "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" ],
      [ settings_plan_path, %w[plan], t("navigation.settings.plan"),
        "M2.25 8.25h19.5M2.25 9h19.5m-16.5 5.25h6m-6 2.25h3m-3.75 3h15a2.25 2.25 0 002.25-2.25V6.75A2.25 2.25 0 0019.5 4.5h-15a2.25 2.25 0 00-2.25 2.25v10.5A2.25 2.25 0 004.5 18.75z" ],
      # API Access (OAuth developer clients) is developer-only — hide it in the
      # native app shell, where managing client secrets / curl snippets is pointless.
      (hotwire_native_app? ? nil : [ settings_api_clients_path, %w[api_clients], t("navigation.settings.items.api_access"), "M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z" ])
    ].compact

    [ [ nil, items ] ]
  end

  # The "Inbox" settings group: one sidebar item per inbox-settings panel, sourced
  # from the shared InboxSettings::Sections catalog so it stays in lockstep with the
  # inbox gear-icon modal. Each item is [path, active_keys, label, icon_path]; the
  # active key is "inbox_<section>" (set by Settings::InboxController#current_section).
  def inbox_settings_nav_group
    items = InboxSettings::Sections::ALL.map do |section|
      [ settings_inbox_section_path(section[:key]),
        [ "inbox_#{section[:key]}" ],
        t(InboxSettings::Sections.label_key(section[:key])),
        InboxSettings::Sections::ICON_PATHS[section[:icon]] ]
    end
    [ t("navigation.settings.groups.inbox"), items ]
  end

  # Renders the section nav for the current area, or nothing when the current
  # controller isn't part of one.
  def section_nav
    here = controller.controller_path
    area = SECTION_AREAS.find { |tabs| tabs.any? { |t| t[:controller] == here } }
    return unless area

    current = area.find { |t| t[:controller] == here }[:key]
    items = area.map { |t| { label: t[:label], href: public_send(t[:path]), key: t[:key] } }
    render(Campbooks::SectionNav.new(items: items, current: current))
  end
end
