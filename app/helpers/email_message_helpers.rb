module EmailMessageHelpers
  def snooze_presets
    now = Time.current
    today_4pm = now.change(hour: 16, min: 0, sec: 0)
    today_4pm += 1.day if today_4pm <= now

    today_8pm = now.change(hour: 20, min: 0, sec: 0)
    today_8pm += 1.day if today_8pm <= now

    tomorrow_9am = 1.day.from_now.change(hour: 9, min: 0, sec: 0)
    saturday_9am = next_weekday(:saturday).in_time_zone.change(hour: 9, min: 0, sec: 0)
    monday_9am   = next_weekday(:monday).in_time_zone.change(hour: 9, min: 0, sec: 0)

    [
      [ :later_today,  t("helpers.snooze.later_today",  time: l(today_4pm, format: :clock)),           today_4pm ],
      [ :this_evening, t("helpers.snooze.this_evening", time: l(today_8pm, format: :clock)),           today_8pm ],
      [ :tomorrow,     t("helpers.snooze.tomorrow",     datetime: l(tomorrow_9am, format: :at_short)), tomorrow_9am ],
      [ :this_weekend, t("helpers.snooze.this_weekend", datetime: l(saturday_9am, format: :at_short)), saturday_9am ],
      [ :next_week,    t("helpers.snooze.next_week",    datetime: l(monday_9am, format: :at_short)),   monday_9am ]
    ]
  end

  def clean_email_address(raw)
    return nil if raw.blank?
    decoded = CGI.unescapeHTML(raw)
    match = decoded.match(/^"?(.+?)"?\s*</)
    if match
      match[1].presence || decoded.split("@").first
    else
      decoded.include?("@") ? decoded.split("@").first : decoded
    end
  end

  def rewrite_email_urls(body, email_account_id)
    return body if body.blank?
    body.gsub(/src=["']\/(?!email_images\/|rails\/)([^"']+)["']/) do
      %(src="/email_images/#{email_account_id}/#{$1}")
    end.gsub(/url\(["']?\/(?!email_images\/|rails\/)([^"')]+)["']?\)/) do
      %(url(/email_images/#{email_account_id}/#{$1}))
    end
  end

  EMAIL_BODY_TAGS = %w[p br a b i u strong em ul ol li blockquote h1 h2 h3 h4 h5 div span table thead tbody tr td th img hr pre code small].freeze
  EMAIL_BODY_ATTRS = %w[href src alt title].freeze

  # Render an email body safely for display. The allowlist sanitizer keeps the
  # TEXT of removed tags, so <style>/<script>/<head> blocks otherwise leak their
  # CSS/JS as visible text (e.g. Outlook/Word "MsoNormal" rules, Zoho "zm_…parse"
  # wrappers). Strip those nodes — content and all — with Loofah first, then
  # sanitize against the allowlist.
  def safe_email_body(raw)
    return "".html_safe if raw.blank?

    fragment = Emails::PlainText.clean_fragment(raw)
    sanitize(fragment.to_html, tags: EMAIL_BODY_TAGS, attributes: EMAIL_BODY_ATTRS)
  end

  # Inner HTML for an email *preview* (e.g. the home-feed card iframe): the latest
  # message only, with quoted reply history removed — <blockquote> citations plus
  # the Gmail/Outlook/Zoho reply header that precedes them, or ">"-prefixed lines
  # in plain-text mail — then sanitised and with inline image URLs rewritten
  # through the proxy. Returns an html_safe fragment ready to drop into an iframe
  # srcdoc body.
  #
  # Sanitises with Loofah's :prune (its HTML5 safelist drops <script>, event
  # handlers and javascript: URLs while keeping — and CSS-sanitising — inline
  # styles, so the email keeps its formatting). Pure Ruby on purpose: no
  # ActionView helpers, so it runs identically as a view helper, inside a Phlex
  # component, and in a Lookbook preview.
  def email_preview_html(message)
    raw = message.body.to_s
    return "".html_safe if raw.blank?

    if raw.match?(/<\w+[^>]*>/)
      fragment = Emails::PlainText.clean_fragment(raw, strip_quotes: true)
      fragment.scrub!(:prune)
      rewrite_email_urls(fragment.to_html, message.email_account_id).html_safe
    else
      %(<div style="white-space:pre-wrap">#{CGI.escapeHTML(Emails::PlainText.strip_text_quotes(raw))}</div>).html_safe
    end
  end

  # Full message body for the OPEN email (detail pane + drawer). Same Loofah
  # :prune sanitisation as #email_preview_html — its HTML5 safelist drops
  # <script>, inline event handlers (onerror=, onload=, onclick=, ...) and
  # javascript: URLs while keeping and CSS-sanitising inline styles — but WITHOUT
  # stripping the quoted reply history, since the open message must show the whole
  # thread. Inline image URLs are rewritten through the proxy. Returns an
  # html_safe fragment.
  #
  # SECURITY: email bodies are attacker-controlled. Never render msg.body with a
  # hand-rolled regex strip + raw() — use this (or #safe_email_body) so the full
  # Loofah safelist runs.
  def safe_email_body_full(message)
    raw = message.body.to_s
    return "".html_safe if raw.blank?

    if raw.match?(/<\w+[^>]*>/)
      fragment = Emails::PlainText.clean_fragment(raw)
      fragment.scrub!(:prune)
      rewrite_email_urls(fragment.to_html, message.email_account_id).html_safe
    else
      %(<div style="white-space:pre-wrap">#{CGI.escapeHTML(raw)}</div>).html_safe
    end
  end

  # Candidates for the discussion composer's @mention autocomplete: every other
  # teammate in the workspace plus Scout (the AI). Shape matches what the
  # mention-autocomplete Stimulus controller expects.
  def mention_candidates_for_composer
    list = [ { name: "Scout", kind: "scout" } ]
    Current.workspace&.users&.each do |user|
      next if user.name.blank? || user == Current.user
      list << { name: user.name, kind: "user" }
    end
    list
  end

  # Match @ followed by 1-4 words (Unicode-aware, supports accented names like João, François).
  # Stops at commas, periods, line breaks, etc.
  MENTION_REGEX = /@([\p{L}][\p{L}\p{N}_\'\-\.]*(?:\s+[\p{L}][\p{L}\p{N}_\'\-\.]*){0,3})/u

  # Bare http(s) URLs in user text → clickable links. Runs on the CGI-escaped comment
  # body before linkify_mentions, so a pasted public file link (/f/:token) is clickable
  # in a comment. Trailing sentence punctuation is left outside the link.
  URL_REGEX = %r{https?://[^\s<>"']+}

  def autolink_urls(escaped_html)
    return escaped_html if escaped_html.blank?

    escaped_html.gsub(URL_REGEX) do |url|
      trimmed = url.sub(/[.,;:!?)\]]+\z/, "")
      trailing = url[trimmed.length..] || ""
      %(<a href="#{trimmed}" target="_blank" rel="noopener noreferrer nofollow" class="text-accent-600 hover:underline">#{trimmed}</a>#{trailing})
    end
  end

  def linkify_mentions(html)
    return html if html.blank?

    doc = Nokogiri::HTML.fragment(html.to_s)
    contacts = mentions_contacts_map
    return html if contacts.empty?

    doc.traverse do |node|
      next unless node.text?
      text = node.text
      next unless text.include?("@")

      inside_link = node.ancestors.any? { |a| a.name == "a" }

      parts = []
      last_end = 0

      text.scan(MENTION_REGEX) do |_|
        match_start = $~.begin(0)
        match_end = $~.end(0)
        name = $1

        parts << CGI.escape_html(text[last_end...match_start]) if match_start > last_end

        entry, count = mention_contact_id(name, contacts)
        if entry
          # The regex greedily grabs up to 4 words after "@"; only the leading
          # words that actually resolve to a target become the chip — any extra
          # trailing words stay as normal text (so "@scout is this spam?" keeps
          # "is this spam?").
          resolved = name.match(/\A(\S+(?:\s+\S+){#{count - 1}})/)[1]
          mention_text = "@#{resolved}"
          parts << (inside_link ? mention_span_html(mention_text, entry) : mention_link_html(mention_text, entry))
          last_end = match_start + 1 + resolved.length
        else
          parts << CGI.escape_html(text[match_start...match_end])
          last_end = match_end
        end
      end

      parts << CGI.escape_html(text[last_end..]) if last_end < text.length
      node.replace(Nokogiri::HTML.fragment(parts.join))
    end

    doc.to_html.html_safe
  end

  # --- Inbox search ---

  # Build the removable active-filter chips for the email search bar. Returns an
  # array of { label:, remove_path: }; each remove_path re-runs the search with
  # that one criterion dropped (arrays drop just the one value). `accounts`/`tags`
  # are passed in so ids can be labelled without an extra query — the search view
  # already loads them.
  def search_active_filters(params, accounts: [], tags: [])
    h = search_param_hash(params)
    chips = []
    label = ->(key, text) { chips << { label: text, remove_path: search_without_param(h, key) } }

    label.call(:q, "“#{h[:q].to_s.truncate(40)}”") if h[:q].present?
    label.call(:folder, t("email_messages.search.chips.folder", value: h[:folder])) if h[:folder].present? && h[:folder] != "all"

    Array(h[:account_ids]).reject(&:blank?).each do |id|
      account = accounts.find { |a| a.id.to_s == id.to_s }
      chips << { label: t("email_messages.search.chips.account", value: account&.email_address || id),
                 remove_path: search_without_param(h, :account_ids, id) }
    end

    Array(h[:tag_ids]).reject(&:blank?).each do |id|
      tag = tags.find { |tg| tg.id.to_s == id.to_s }
      chips << { label: t("email_messages.search.chips.tag", value: tag&.name || id),
                 remove_path: search_without_param(h, :tag_ids, id) }
    end

    label.call(:sender, t("email_messages.search.chips.sender", value: h[:sender])) if h[:sender].present?
    label.call(:domain, t("email_messages.search.chips.domain", value: h[:domain])) if h[:domain].present?
    label.call(:date_from, t("email_messages.search.chips.date_from", value: h[:date_from])) if h[:date_from].present?
    label.call(:date_to, t("email_messages.search.chips.date_to", value: h[:date_to])) if h[:date_to].present?
    label.call(:has_attachment, t("email_messages.search.chips.has_attachment")) if h[:has_attachment].to_s == "1"
    label.call(:unread, t("email_messages.search.chips.unread")) if h[:unread].to_s == "1"
    label.call(:category, t("email_messages.search.chips.category", value: h[:category].to_s.humanize)) if h[:category].present?
    label.call(:priority, t("email_messages.search.chips.priority", value: h[:priority].to_s.humanize)) if h[:priority].present?

    chips
  end

  # The email_messages/search path with one criterion removed (pagination always
  # reset). For an array param, pass `value` to drop just that entry.
  def search_without_param(params, key, value = nil)
    h = search_param_hash(params)
    h.delete(:page)

    if value && h[key].is_a?(Array)
      h[key] = h[key].reject { |v| v.to_s == value.to_s }
      h.delete(key) if h[key].empty?
    else
      h.delete(key)
    end

    search_email_messages_path(h)
  end

  # Default/floor width (px) for the inbox thread-list pane. The base is `w-64`
  # (16rem = 224px at the app's 14px root). The per-account filter renders one
  # ~22px avatar per linked account in the header — but only when 2+ accounts are
  # linked — which at the base width clips the "Inbox" label. So widen the default
  # by the filter's footprint; capped so many accounts can't make it unwieldy.
  # The pane can still be dragged down to the base width (panel-resize min).
  INBOX_LIST_BASE_PX = 224
  INBOX_LIST_PER_ACCOUNT_PX = 22
  INBOX_LIST_MAX_DEFAULT_PX = 420

  def inbox_list_width_px(accounts)
    count = Array(accounts).size
    extra = count >= 2 ? count * INBOX_LIST_PER_ACCOUNT_PX : 0
    [ INBOX_LIST_BASE_PX + extra, INBOX_LIST_MAX_DEFAULT_PX ].min
  end

  private

  def search_param_hash(params)
    raw = params.respond_to?(:to_unsafe_h) ? params.to_unsafe_h : params.to_h
    raw.deep_symbolize_keys
  end

  # Targets a @mention can resolve to, keyed by lowercased name. Contacts come
  # first; workspace teammates override a same-named contact (in a discussion you
  # tag a colleague, not an email contact); "scout" is reserved for the AI and
  # always wins. Each entry carries a :kind so it renders the right chip.
  def mentions_contacts_map
    @_mentions_contacts_map ||= begin
      map = {}
      Contact.where.not(name: nil).where.not(name: "")
        .pluck(:name, :id, :email)
        .each { |name, id, email| map[name.downcase.strip] = { kind: :contact, id: id, name: name, email: email } }
      Current.workspace&.users&.each do |user|
        next if user.name.blank?
        map[user.name.downcase.strip] = { kind: :user, id: user.id, name: user.name, email: user.email_address }
      end
      map["scout"] = { kind: :scout, name: "Scout" }
      map
    end
  end

  # Resolve the longest leading run of words (1..N) that names a target. Returns
  # [entry, word_count] so the caller can chip only the resolved words.
  def mention_contact_id(name, contacts_map)
    words = name.split(/\s+/)
    count = words.length
    while count.positive?
      candidate = words.first(count).join(" ").downcase
      return [ contacts_map[candidate], count ] if contacts_map[candidate]

      last = words[count - 1]
      if last.end_with?("'s")
        stripped = (words.first(count - 1) + [ last.sub(/'s$/, "") ]).join(" ").downcase
        return [ contacts_map[stripped], count ] if contacts_map[stripped]
      end
      count -= 1
    end
    [ nil, 0 ]
  end

  POPOVER_ATTRS = "data-controller=\"contact-popover\" data-action=\"mouseenter->contact-popover#mouseEnter mouseleave->contact-popover#mouseLeave\""

  # Two entry points kept for the inside-a-link vs not distinction (contacts
  # render as <a>, which can't nest inside another <a>). Teammate and Scout chips
  # are always <span>, so they're safe in either position.
  def mention_link_html(mention_text, entry)
    case entry[:kind]
    when :scout then scout_mention_html
    when :user  then user_mention_html(mention_text, entry)
    else contact_mention_link_html(mention_text, entry)
    end
  end

  def mention_span_html(mention_text, entry)
    case entry[:kind]
    when :scout then scout_mention_html
    when :user  then user_mention_html(mention_text, entry)
    else contact_mention_span_html(mention_text, entry)
    end
  end

  def contact_mention_link_html(mention_text, contact_entry)
    id = contact_entry[:id]
    initial = avatar_initial(contact_entry)
    name_text = CGI.escape_html(mention_text.sub(/\A@/, ""))
    %(<a href="/contacts/#{id}" class="inline-flex items-center gap-1 align-middle no-underline leading-none rounded-full pl-1 pr-2 py-0.5 bg-accent-50 hover:bg-accent-100 transition-colors" #{POPOVER_ATTRS} data-contact-popover-contact-id-value="#{id}" data-turbo-frame="_top"><span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-100 text-accent-600 text-[9px] font-semibold flex-shrink-0">#{initial}</span><span class="text-accent-600 font-medium">#{name_text}</span></a>)
  end

  def contact_mention_span_html(mention_text, contact_entry)
    id = contact_entry[:id]
    initial = avatar_initial(contact_entry)
    name_text = CGI.escape_html(mention_text.sub(/\A@/, ""))
    %(<span class="inline-flex items-center gap-1 align-middle leading-none rounded-full pl-1 pr-2 py-0.5 bg-accent-50" #{POPOVER_ATTRS} data-contact-popover-contact-id-value="#{id}"><span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-100 text-accent-600 text-[9px] font-semibold flex-shrink-0">#{initial}</span><span class="text-accent-600 font-medium underline decoration-dotted">#{name_text}</span></span>)
  end

  # A teammate chip: accent-tinted with an initial avatar. No link target — there
  # is no member profile page — so it's a plain span.
  def user_mention_html(mention_text, entry)
    initial = avatar_initial(entry)
    name_text = CGI.escape_html(mention_text.sub(/\A@/, ""))
    %(<span class="inline-flex items-center gap-1 align-middle leading-none rounded-full pl-1 pr-2 py-0.5 bg-accent-100 dark:bg-accent-500/15"><span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-accent-200 text-accent-700 dark:bg-accent-500/30 dark:text-accent-100 text-[9px] font-semibold flex-shrink-0">#{initial}</span><span class="text-accent-700 dark:text-accent-200 font-medium">#{name_text}</span></span>)
  end

  SCOUT_MENTION_SPARKLE = %(<svg class="w-3 h-3 flex-shrink-0" fill="currentColor" viewBox="0 0 24 24"><path d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5z"/></svg>)

  # Scout reads as the AI: a solid violet chip with a sparkle, distinct from the
  # tinted teammate chips. Always shows "Scout" regardless of how it was typed.
  def scout_mention_html
    %(<span class="inline-flex items-center gap-1 align-middle leading-none rounded-full pl-1.5 pr-2 py-0.5 bg-accent-500 text-white font-medium">#{SCOUT_MENTION_SPARKLE}Scout</span>)
  end

  def avatar_initial(contact_entry)
    base = contact_entry[:name].presence || contact_entry[:email].to_s
    CGI.escape_html((base.presence || "?")[0].upcase)
  end

  def next_weekday(day_name)
    day_map = { sunday: 0, monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6 }
    target = day_map[day_name] || 0
    current = Date.current.wday
    diff = (target - current) % 7
    diff = 7 if diff == 0
    Date.current + diff.days
  end
end
