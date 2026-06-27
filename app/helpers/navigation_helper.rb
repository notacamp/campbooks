module NavigationHelper
  # Primary areas that expose sub-sections through the contextual section nav
  # (the thin bar under the topbar). Each area is an ordered list of tabs;
  # a tab maps a controller to a label and an index path. The bar renders
  # whenever the current controller belongs to one of these areas.
  # Document-type management moved into the inbox settings modal, so the Docs
  # area no longer needs a contextual tab bar. Kept as an extension point.
  SECTION_AREAS = [].freeze

  # Inline SVG bodies for the primary nav icons (rendered raw, mirroring the
  # Campbooks::Logo component's approach). Scout is a filled spark; the rest are
  # stroked line icons sharing one visual weight.
  NAV_ICON_PATHS = {
    home: '<path d="M3 11l9-8 9 8"/><path d="M5 10v10h5v-6h4v6h5V10"/>',
    mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
    documents: '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/>',
    workflows: '<circle cx="6" cy="6" r="2.4"/><circle cx="6" cy="18" r="2.4"/><circle cx="18" cy="12" r="2.4"/><path d="M8 7.4 16 11M8 16.6 16 13"/>',
    calendar: '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>',
    contacts: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
    activity: '<path d="M3 12h4l2 6 4-13 2 7h6"/>'
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
  # item: { key:, label:, path:, ember:, active: }. Scout is the one Ember item
  # (the Meaning Rule — Ember means Scout / live / win); the active section is
  # rendered in near-black ink, never Ember. Admin/Settings are intentionally
  # absent here: they live in the avatar menu, not the primary nav.
  def primary_nav_items
    [
      nav_item(:home, t("shared.nav.home"), root_path, exact: true, also_active_for: [ home_path ], badge: nav_attention.dot?(:home)),
      nav_item(:mail, t("shared.nav.mail"), email_messages_path, badge: nav_attention.dot?(:mail)),
      nav_item(:calendar, t("shared.nav.calendar"), calendar_path, badge: nav_attention.dot?(:calendar)),
      nav_item(:scout, t("shared.nav.scout"), scout_path, ember: true, badge: nav_attention.dot?(:scout)),
      nav_item(:documents, t("shared.nav.documents"), documents_path, badge: nav_attention.dot?(:documents)),
      # Workflows is gated off by default until it's production-ready (Features.workflows?).
      (nav_item(:workflows, t("shared.nav.workflows"), workflows_path) if Features.workflows?),
      nav_item(:contacts, t("shared.nav.contacts"), contacts_path),
      nav_item(:activity, t("shared.nav.activity"), activity_path)
    ].compact
  end

  # Memoized per request: the "action required" dots for the primary nav, read by
  # primary_nav_items above and rendered by Campbooks::NavRail / BottomNav.
  def nav_attention
    @nav_attention ||= Navigation::Attention.new(current_user)
  end

  # One nav item with its computed active state. Active matching mirrors the
  # legacy nav_link: exact for Home (every path starts with "/"), prefix for the
  # rest (so /email_messages/123 keeps Mail lit).
  def nav_item(key, label, path, ember: false, exact: false, also_active_for: [], badge: false)
    candidates = [ path ] + Array(also_active_for)
    active = exact ? candidates.include?(request.path) : candidates.any? { |p| request.path.start_with?(p) }
    { key: key, label: label, path: path, ember: ember, active: active, badge: badge }
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

  # Grouped settings navigation, shared by the settings sidebar
  # (app/views/settings/_sidebar.html.erb) and the topbar user menu
  # (app/views/shared/_user_menu.html.erb) so both expose the same sections in
  # the same order. Each group is [heading, items]; each item is
  # [path, active_keys, label, icon_path]. A sidebar link is active when
  # current_section is in its active_keys list (the user menu ignores active_keys
  # since it renders outside the settings controllers).
  def settings_nav_groups
    [
      [ t("navigation.settings.groups.workspace"), [
        [ settings_root_path, %w[general], t("navigation.settings.items.general"), "M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" ],
        [ settings_plan_path, %w[plan], t("navigation.settings.items.plan"), "M2.25 8.25h19.5M2.25 9h19.5m-16.5 5.25h6m-6 2.25h3m-3.75 3h15a2.25 2.25 0 002.25-2.25V6.75A2.25 2.25 0 0019.5 4.5h-15a2.25 2.25 0 00-2.25 2.25v10.5A2.25 2.25 0 004.5 18.75z" ],
        [ settings_members_path, %w[members], t("navigation.settings.items.members"), "M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" ],
        [ settings_integrations_root_path, %w[integrations notion google_drive zoho_drive calendars], t("navigation.settings.items.integrations"), "M11 4a2 2 0 114 0v1a1 1 0 001 1h3a1 1 0 011 1v3a1 1 0 01-1 1h-1a2 2 0 100 4h1a1 1 0 011 1v3a1 1 0 01-1 1h-3a1 1 0 01-1-1v-1a2 2 0 10-4 0v1a1 1 0 01-1 1H7a1 1 0 01-1-1v-3a1 1 0 00-1-1H4a2 2 0 110-4h1a1 1 0 001-1V7a1 1 0 011-1h3a1 1 0 001-1V4z" ],
        [ settings_api_clients_path, %w[api_clients], t("navigation.settings.items.api_access"), "M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z" ],
        [ settings_data_privacy_path, %w[data_privacy], t("navigation.settings.items.data_privacy"), "M16.5 10.5V6.75a4.5 4.5 0 10-9 0v3.75m-.75 11.25h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" ]
      ] ],
      [ t("navigation.settings.groups.ai_and_automation"), [
        [ settings_ai_path, %w[ai], t("navigation.settings.items.ai_providers_and_services"), "M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.456-2.456L14.25 6l1.035-.259a3.375 3.375 0 002.456-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 00-2.456 2.456z" ],
        [ settings_pipelines_path, %w[pipelines], t("navigation.settings.items.pipelines"), "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" ]
      ] ],
      [ t("navigation.settings.groups.your_account"), [
        [ settings_account_path, %w[account], t("navigation.settings.items.account"), "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" ],
        [ settings_security_path, %w[security totp passkeys recovery_codes email_otp audit_log], t("navigation.settings.items.security"), "M9 12.75 11.25 15 15 9.75m-3-7.036A11.959 11.959 0 0 1 3.598 6 11.99 11.99 0 0 0 3 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285Z" ],
        [ settings_notifications_path, %w[notifications], t("navigation.settings.items.notifications"), "M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" ]
      ] ]
    ]
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
