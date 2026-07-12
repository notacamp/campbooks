module ApplicationHelper
  include Pagy::Frontend
  include EmailMessageHelpers

  # `also_active_for` keeps a primary nav item highlighted while you're on one
  # of its sub-sections (e.g. "Docs" stays active on /document_types).
  def nav_link(text, path, also_active_for: [])
    active = if path == "/"
      current_page?(path)
    else
      ([ path ] + Array(also_active_for)).any? { |p| request.path.start_with?(p) }
    end
    classes = if active
      "inline-flex items-center px-2.5 py-0.5 rounded-md text-[13px] font-medium bg-accent-50 text-accent-700"
    else
      "inline-flex items-center px-2.5 py-0.5 rounded-md text-[13px] font-medium text-gray-500 hover:text-gray-900 hover:bg-gray-50"
    end
    link_to text, path, class: classes
  end

  # JSON catalog of navigation/settings/action commands for the Cmd+K palette.
  # Returned as a plain (non-html_safe) string so ERB escapes it into the data
  # attribute; the browser decodes it and Stimulus parses it as an Array value.
  def command_palette_catalog_json
    return "[]" unless authenticated?

    CommandPaletteCatalog.for(Current.user).to_json
  end

  # Native language names for the settings switcher — always shown in their own
  # language (endonyms), so each reads correctly whatever the current UI locale.
  LOCALE_NATIVE_NAMES = {
    "en" => "English",
    "pt" => "Português",
    "es" => "Español",
    "fr" => "Français"
  }.freeze

  # [[label, value]] pairs for the language <select>, ordered by available_locales.
  def locale_options
    I18n.available_locales.map { |loc| [ LOCALE_NATIVE_NAMES[loc.to_s] || loc.to_s, loc.to_s ] }
  end

  # Composes the document <title>: "<page> · Campbooks" when a view set a title
  # via content_for(:title), otherwise just the product name. Page titles are
  # translated in their own views; the product name is a brand constant.
  def page_title(brand = "Campbooks")
    content_for?(:title) ? "#{content_for(:title)} · #{brand}" : brand
  end

  # Canonical marketing-site URL. Privacy Policy / Terms live on the public site
  # (not the app), so the registration consent + in-app legal links point here.
  # Override with MARKETING_BASE_URL to point at your own site when self-hosting.
  MARKETING_BASE_URL = ENV.fetch("MARKETING_BASE_URL", "https://campbooks.not-a-camp.com").freeze

  def marketing_url(path = "")
    "#{MARKETING_BASE_URL}#{path}"
  end

  # URL for the public REST API reference, linked from Settings → API access.
  # Defaults to the source-available guide on GitHub so self-hosters always have
  # a working link; the hosted cloud overrides API_DOCS_URL to point at the
  # rendered reference on the docs site.
  API_DOCS_URL = ENV.fetch(
    "API_DOCS_URL", "https://github.com/notacamp/campbooks/blob/main/docs/api.md"
  ).freeze

  def api_docs_url
    API_DOCS_URL
  end

  # Localized label for an enum value, looked up at
  # activerecord.attributes.<model>.<attribute_plural>.<value> (e.g.
  # activerecord.attributes.document.statuses.processed). Falls back to a
  # humanized value so an un-extracted enum never blanks out or raises.
  def human_enum(model_class, attribute, value)
    return "" if value.blank?
    I18n.t(
      "#{attribute.to_s.pluralize}.#{value}",
      scope: [ :activerecord, :attributes, model_class.model_name.i18n_key ],
      default: value.to_s.humanize
    )
  end

  def status_badge(status)
    # Harmonized OKLCH tones (.tone-* in application.css), grouped by meaning.
    tones = {
      "pending" => "tone-amber", "fetched" => "tone-amber",
      "processing" => "tone-blue", "running" => "tone-blue", "generating" => "tone-blue",
      "processed" => "tone-green", "completed" => "tone-green", "generated" => "tone-green",
      "review" => "tone-orange",
      "approved" => "tone-green", "sent" => "tone-green",
      "failed" => "tone-red",
      "rejected" => "tone-neutral", "ignored" => "tone-neutral"
    }

    tone = tones[status.to_s] || "tone-neutral"
    label = t("statuses.#{status}", default: status.to_s.humanize)
    content_tag(:span, label, class: "inline-flex items-center rounded-md px-2.5 py-0.5 text-xs font-medium #{tone}")
  end

  def document_type_name(type)
    return unless type
    type.respond_to?(:name) ? type.name : type.to_s
  end

  def document_type_badge_style(type)
    color = document_type_dot_color(type)
    "color: #{color}; background-color: #{color}20"
  end

  def document_type_dot_color(type)
    return "#6b7280" unless type
    type.respond_to?(:color) && type.color.presence || "#6b7280"
  end

  def document_type_label(type)
    return t("helpers.document_type.unclassified") unless type
    name = type.respond_to?(:name) ? type.name : type.to_s
    name.humanize
  end

  def format_currency(cents, currency = "EUR")
    return "-" if cents.nil?
    Money.new(cents, currency).format
  end

  def payment_method_options
    Document::PAYMENT_METHODS.map { |key| [ t("helpers.payment_methods.#{key}"), key ] }
  end

  def payment_method_label(method)
    t("helpers.payment_methods.#{method}", default: method.to_s.humanize)
  end

  def date_section_label(date)
    return unless date
    today = Date.current
    d = date.to_date

    if d == today
      t("helpers.date.today")
    elsif d >= today.beginning_of_week
      t("helpers.date.this_week")
    elsif d >= today.beginning_of_month
      t("helpers.date.this_month")
    elsif d >= (today - 1.month).beginning_of_month
      t("helpers.date.last_month")
    else
      l(date, format: :section)
    end
  end

  # Locale-independent slug for a date's section, mirroring the buckets in
  # date_section_label. Used as a stable DOM key so a section's checkbox and its
  # rows match across renders (and infinite-scroll pages) regardless of locale.
  def date_section_key(date)
    return unless date
    today = Date.current
    d = date.to_date

    if d == today
      "today"
    elsif d >= today.beginning_of_week
      "this-week"
    elsif d >= today.beginning_of_month
      "this-month"
    elsif d >= (today - 1.month).beginning_of_month
      "last-month"
    else
      date.strftime("%Y-%m")
    end
  end

  def thread_date_label(date)
    return unless date
    if date.to_date == Date.current
      l(date, format: :thread)
    else
      l(date, format: :day_month)
    end
  end

  def folder_icon(name, active = false)
    icon = case name
    when "Inbox" then inbox_svg
    when "Sent" then sent_svg
    when "Drafts" then drafts_svg
    when "Archive", "INBOX Archive" then archive_svg
    when "Spam" then spam_svg
    when "Trash" then trash_svg
    when "Snoozed" then snoozed_svg
    when "Starred" then star_svg
    when "All" then all_mail_svg
    else default_folder_svg
    end
    color = active ? "text-accent-600" : "text-gray-400"
    "<span class=\"#{color}\">#{icon}</span>".html_safe
  end

  def folder_icon_small(name)
    icon = case name
    when "Inbox" then inbox_svg
    when "Sent" then sent_svg
    when "Drafts" then drafts_svg
    when "Archive", "INBOX Archive" then archive_svg
    when "Spam" then spam_svg
    when "Trash" then trash_svg
    when "All Mail", "All" then all_mail_svg
    else default_folder_svg
    end
    icon.html_safe
  end

  def grouped_threads(threads, current_group: nil, tag_groups: nil)
    if current_group.present?
      return date_grouped(threads)
    end

    date_grouped(threads)
  end

  # The date-section label of the last thread on a page. Threads are ordered newest
  # first, so this is the section the next (older) page continues from — passed along
  # the infinite-scroll request so the appended page can suppress a duplicate header.
  def thread_continue_section(threads)
    last = threads.to_a.last
    last && date_section_label(last.latest_message&.received_at)
  end

  def group_items(tag_groups, threads)
    items = []
    thread_ids_in_groups = Set.new
    group_senders = Hash.new { |h, k| h[k] = [] }

    threads.each do |thread|
      latest = thread.latest_message
      next unless latest

      # Collect tags from all messages in the thread, not just the latest
      all_tag_names = if thread.email_messages.loaded?
        thread.email_messages.flat_map { |m| m.tags.loaded? ? m.tags.map(&:name) : m.tags.pluck(:name) }.uniq
      else
        thread.email_messages.joins(:tags).pluck("tags.name").uniq
      end

      tag_groups.each do |group_name, data|
        group_tag_names = data[:tags].map(&:name)
        if (all_tag_names & group_tag_names).any?
          thread_ids_in_groups << thread.id
          existing = group_senders[group_name]
          if existing.length < 3 && !existing.any? { |s| s[:email] == latest.from_address }
            existing << {
              email: latest.from_address,
              contact_id: latest.contact_id,
              sent: latest.sent?,
              account_color: thread.email_account&.color
            }
          end
        end
      end
    end

    tag_groups.each do |group_name, data|
      items << { type: :group, label: group_name, count: data[:count], senders: group_senders[group_name], color: data[:tags].first&.color }
    end

    items
  end

  private

  def date_grouped(threads)
    sections = {}
    threads.each do |thread|
      latest = thread.latest_message
      next unless latest&.received_at
      key = date_section_key(latest.received_at)
      sections[key] ||= { key: key, label: date_section_label(latest.received_at), threads: [] }
      sections[key][:threads] << thread
    end
    sections.values
  end

  public

  def provider_logo(provider, size: :md)
    dims = size == :lg ? "w-10 h-10" : "w-8 h-8"
    inner_dims = size == :lg ? "w-5 h-5" : "w-4 h-4"
    svg = case provider.to_s.downcase
    when "zoho"
      "<svg class=\"#{inner_dims}\" viewBox=\"0 0 24 24\" fill=\"none\"><rect width=\"24\" height=\"24\" rx=\"6\" fill=\"#E53E3E\"/><text x=\"12\" y=\"17\" text-anchor=\"middle\" font-family=\"Arial,sans-serif\" font-weight=\"bold\" font-size=\"14\" fill=\"white\">Z</text></svg>"
    when "google", "gmail"
      "<svg class=\"#{inner_dims}\" viewBox=\"0 0 24 24\" fill=\"none\"><rect width=\"24\" height=\"24\" rx=\"6\" fill=\"#EA4335\"/><path d=\"M6 8l6 4 6-4\" stroke=\"white\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"/><rect x=\"5\" y=\"7\" width=\"14\" height=\"11\" rx=\"2\" stroke=\"white\" stroke-width=\"1.5\" fill=\"none\"/></svg>"
    when "microsoft"
      "<svg class=\"#{inner_dims}\" viewBox=\"0 0 24 24\" fill=\"none\"><rect width=\"24\" height=\"24\" rx=\"6\" fill=\"#0078D4\"/><rect x=\"6\" y=\"6\" width=\"5\" height=\"5\" rx=\"1\" fill=\"#F25022\"/><rect x=\"13\" y=\"6\" width=\"5\" height=\"5\" rx=\"1\" fill=\"#7FBA00\"/><rect x=\"6\" y=\"13\" width=\"5\" height=\"5\" rx=\"1\" fill=\"#00A4EF\"/><rect x=\"13\" y=\"13\" width=\"5\" height=\"5\" rx=\"1\" fill=\"#FFB900\"/></svg>"
    else
      "<svg class=\"#{inner_dims}\" viewBox=\"0 0 24 24\" fill=\"none\"><rect width=\"24\" height=\"24\" rx=\"6\" fill=\"#6B7280\"/><path d=\"M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z\" stroke=\"white\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"/></svg>"
    end
    "<span class=\"inline-flex items-center justify-center #{dims} rounded-xl flex-shrink-0\" style=\"background-color: #{provider_bg(provider)}\">#{svg}</span>".html_safe
  end

  def provider_bg(provider)
    case provider.to_s.downcase
    when "zoho" then "#FFF5F5"
    when "google", "gmail" then "#FCE8E6"
    when "microsoft" then "#E6F0FA"
    else "#F3F4F6"
    end
  end

  def provider_name(provider)
    case provider.to_s.downcase
    when "zoho" then "Zoho Mail"
    when "google", "gmail" then "Gmail"
    when "microsoft" then "Microsoft 365"
    else provider.to_s.humanize
    end
  end

  private

  def inbox_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'
  end

  def sent_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"/></svg>'
  end

  def drafts_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/></svg>'
  end

  def archive_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/></svg>'
  end

  def spam_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636"/></svg>'
  end

  def trash_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>'
  end

  def snoozed_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'
  end

  def star_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.915a1 1 0 00.95-.69l1.519-4.674z"/></svg>'
  end

  def all_mail_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>'
  end

  def default_folder_svg
    '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>'
  end

  # Pretty-prints a body string for display in the admin call detail view.
  # If the string parses as JSON, returns an indented representation; otherwise
  # returns the raw string. The caller must escape this with ERB (no html_safe).
  def pretty_body(body)
    return "" if body.blank?
    parsed = JSON.parse(body)
    JSON.pretty_generate(parsed)
  rescue JSON::ParserError, TypeError
    body
  end

  def render_markdown(text)
    return "" if text.blank?
    # Output is marked html_safe, so strip any raw HTML / unsafe-scheme links in
    # the source (defense-in-depth: only call this with trusted text regardless).
    @markdown ||= ::Redcarpet::Markdown.new(
      ::Redcarpet::Render::HTML.new(filter_html: true, safe_links_only: true),
      autolink: true,
      no_intra_emphasis: true,
      tables: true
    )
    @markdown.render(text).html_safe
  end
end
