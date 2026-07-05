# frozen_string_literal: true

module Campbooks
  module Feed
    # Shared base for the home-feed cards. Holds the presentation helpers for the
    # email/document subject (content is rendered live from the record, never
    # cached) and the small POST forms that wire a card's actions to
    # Feed::ItemsController — reusing the same CSRF pattern as Campbooks::ChatActions.
    #
    # Concrete cards take the same (item:, subject:) pair and compose these.
    class Base < Campbooks::Base
      # Enough of the email to fill the ~10-line clamp and give "Read more"
      # something to reveal, without dumping a whole newsletter into the feed.
      EXPANDED_EXCERPT_LENGTH = 1500

      def initialize(item:, subject:, **attrs)
        @item = item
        @subject = subject
        @attrs = attrs
      end

      private

      attr_reader :item, :subject

      # --- action forms ----------------------------------------------------------

      # A submit button wrapped in POST /feed/items/:id/act. Turbo turns the
      # response (remove the card + raise a toast) into an inline update, disables
      # the button during the request (no double-submit) and swaps in a pending
      # label. `hint` becomes a title tooltip explaining the action's consequence.
      #
      # `key:`/`primary:`/`dismiss:` wire the button into the keyboard layer
      # (feed_keyboard_controller): the focused card's → fires its primary, ← its
      # escape, a letter its `data-feed-key`. They also surface the matching chip
      # (see #key_chip) on the focused card. Mobile reaches the same actions by
      # swiping (Feed::Card configures the Swipeable from the same tools).
      def act_button(tool:, label:, args: {}, variant: :ghost, size: :sm, hint: nil, key: nil, primary: false, dismiss: false)
        action_form(helpers.act_feed_item_path(item), fields: { tool: tool.to_s }.merge(arg_fields(args))) do
          render Campbooks::Button.new(
            variant: variant, size: size, type: "submit", title: hint,
            data: feed_action_attrs(key:, primary:, dismiss:).merge(turbo_submits_with: t("components.feed.shared.working"))
          ) do
            plain label
            key_chip(key:, primary:, dismiss:)
          end
        end
      end

      # "Not now" — hides just this card (POST /feed/items/:id/dismiss). The
      # card's escape by default (← / swipe-left), so `dismiss:` defaults true.
      def dismiss_button(label:, variant: :ghost, size: :sm, hint: nil, key: nil, dismiss: true)
        action_form(helpers.dismiss_feed_item_path(item)) do
          render Campbooks::Button.new(
            variant: variant, size: size, type: "submit", title: hint,
            data: feed_action_attrs(key:, dismiss:).merge(turbo_submits_with: t("components.feed.shared.working"))
          ) do
            plain label
            key_chip(key:, dismiss:)
          end
        end
      end

      # Navigation (renders <a>), for actions that need the full email/document
      # surface — "Open", "Reply", "Fix" — rather than a one-tap mutation. These are
      # a card's primary, so `primary:` defaults true (→ / Enter / o).
      def link_button(href:, label:, variant: :primary, size: :sm, key: nil, primary: true, dismiss: false)
        render Campbooks::Button.new(
          variant: variant, size: size, href: href,
          data: feed_action_attrs(key:, primary:, dismiss:)
        ) do
          plain label
          key_chip(key:, primary:, dismiss:)
        end
      end

      def action_form(url, fields: {}, &block)
        form(action: url, method: :post, class: "inline-flex", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          fields.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          yield
        end
      end

      def arg_fields(args)
        (args || {}).transform_keys { |k| "args[#{k}]" }
      end

      # The data hooks feed_keyboard_controller binds to. The arrows act on
      # [data-feed-primary] / [data-feed-dismiss]; a letter on [data-feed-key].
      def feed_action_attrs(key: nil, primary: false, dismiss: false)
        attrs = {}
        attrs[:feed_primary] = true if primary
        attrs[:feed_dismiss] = true if dismiss
        attrs[:feed_key] = key if key
        attrs
      end

      # A small key-cap appended inside a feed action button. It surfaces only on
      # the scroll-focused card and only on fine-pointer devices (CSS gates
      # [data-feed-keyhint]); elsewhere it's display:none and costs no space. The
      # glyph mirrors how the action is reached — → primary, ← escape, else the
      # literal letter — so the card teaches its own shortcuts as you land on it.
      def key_chip(key: nil, primary: false, dismiss: false)
        glyph = chip_glyph(key:, primary:, dismiss:)
        return unless glyph

        kbd(
          data: { feed_keyhint: true }, aria: { hidden: "true" },
          class: "ml-1.5 -mr-0.5 items-center justify-center rounded border border-current px-1 " \
                 "font-mono text-[10px] font-semibold leading-[1.4] opacity-60"
        ) { glyph }
      end

      def chip_glyph(key: nil, primary: false, dismiss: false)
        return "→" if primary
        return "←" if dismiss

        key&.upcase
      end

      # --- email presentation ----------------------------------------------------

      # Strip rendering gremlins from real-world data: invalid byte sequences and
      # any U+FFFD replacement chars left by a lossy ingest (e.g. a mangled flag
      # emoji), so a corrupted subject doesn't print a black diamond.
      def safe_text(value)
        # Decode HTML entities the email pipeline left in summaries/subjects (so
        # "&quot;" shows as a real quote, not literal text — Phlex re-escapes for
        # output safety), scrub invalid bytes, and drop U+FFFD mojibake.
        CGI.unescapeHTML(value.to_s.scrub("")).delete("\u{FFFD}")
      end

      # "Display Name <addr>" → "Display Name"; bare address → humanized local part.
      def sender_name(message)
        name_from_address(message.from_address).presence || t("components.feed.shared.unknown_sender")
      end

      def name_from_address(address)
        from = safe_text(address).strip
        return "" if from.blank?

        if from =~ /\A\s*"?([^"<]+?)"?\s*<.*>\s*\z/
          Regexp.last_match(1).strip
        else
          from.split("@").first.to_s.tr("._", " ").split.map(&:capitalize).join(" ").presence || from
        end
      end

      # The distinct senders of a message's thread for the facepile, newest first.
      # Falls back to just this message's sender when it has no thread. `known_count`
      # (the card's stamped thread_count) skips the participant query for a
      # single-message thread — the common case — so the feed avoids an N+1.
      def thread_participants(message, limit: 6, known_count: nil)
        solo = [ { email: message.from_address, contact_id: message.contact_id } ]
        return solo if known_count && known_count <= 1

        thread = message.email_thread
        return solo unless thread

        thread.participant_senders(limit: limit)
      end

      # "Ana" · "Ana & Bob" · "Ana, Bob & 3 others" — the facepile's name line.
      def participants_label(participants)
        names = participants.map { |p| name_from_address(p[:email]) }.reject(&:blank?).uniq
        case names.size
        when 0 then t("components.feed.shared.unknown_sender")
        when 1 then names.first
        when 2 then t("components.feed.shared.two_participants", first: names[0], second: names[1])
        else t("components.feed.shared.many_participants", names: names.first(2).join(", "), count: names.size - 2)
        end
      end

      # Strip the noise the AI was supposed to handle: leading Re:/Fwd: and any
      # run of bracketed prefixes (ticket refs like "[## 3792 ##]", list tags).
      def clean_subject(message)
        clean_subject_text(message.subject)
      end

      # Same cleaning for a bare subject string (e.g. one stamped into a card's
      # item data rather than read live off a record).
      def clean_subject_text(subject)
        safe_text(subject)
          .sub(/\A(?:(?:re|fwd?):\s*|\[[^\]]*\]\s*)+/i, "")
          .strip.presence || t("components.feed.shared.no_subject")
      end

      def excerpt(message, length: 200)
        text = message.ai_summary.presence || message.summary.presence || helpers.strip_tags(message.body.to_s)
        safe_text(text).squish.truncate(length)
      end

      # The email body for a card. Renders the real HTML in a sandboxed iframe so
      # the sender's formatting survives (Campbooks::EmailHtmlPreview), falling
      # back to a clamped plain-text excerpt when there's no HTML body to show.
      def email_body_preview(message, margin:)
        if message.body.present?
          render Campbooks::EmailHtmlPreview.new(message: message, class: margin)
        else
          render Campbooks::ClampText.new(lines: 10, class: "#{margin} text-sm leading-relaxed text-muted-foreground") do
            excerpt(message, length: EXPANDED_EXCERPT_LENGTH)
          end
        end
      end

      # Compact, localized timestamp (via rails-i18n :short/:long formats).
      def relative_time(time)
        return "" if time.nil?

        date = time.to_date
        if date == Date.current
          l(time, format: :short)
        elsif date == Date.current - 1
          t("components.feed.shared.yesterday")
        else
          l(date, format: :long)
        end
      end

      # A small Ember dot is the sanctioned priority accent (DESIGN.md: Ember means
      # Scout / live / a win — priority qualifies; a colored block would not).
      def priority_dot
        span(
          class: "h-2 w-2 flex-shrink-0 rounded-full",
          style: "background-color: var(--ember-solid)",
          aria: { label: t("components.feed.shared.priority") },
          role: "img"
        )
      end
    end
  end
end
