# frozen_string_literal: true

module Campbooks
  module Feed
    # Home-feed / Now-deck card for an actionable notification (a "Needs you"
    # notice): a category glyph, the notice's title + body, when it arrived, and
    # three actions — Open (its link_url, the primary; a category-specific verb
    # like Reconnect / Review when obvious), Done (mark read + archive, via the
    # feed act endpoint's `notice` branch), and Later (dismiss just this card).
    #
    # The subject is a Notification. Left-swipe is Later (dismiss); the primary is
    # navigation, so there's no right-swipe (tap Open) — same shape as the digest
    # card. Non-reversible, so Done raises a plain success toast.
    class NoticeCard < Campbooks::Feed::Base
      # A category → inline glyph map. Stroked to match the app's icon set; the
      # bell is the honest default (it is, after all, a notification).
      ICONS = {
        system: '<path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/>',
        document: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/>',
        reconciliation: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/>',
        task: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="m8 12 2.5 2.5L16 9"/>',
        export: '<path d="M12 3v12m0 0 4-4m-4 4-4-4M5 21h14"/>'
      }.freeze
      DEFAULT_ICON = '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 7 3 9H3c0-2 3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/>'

      def view_template
        div(class: "-mx-3 flex flex-col gap-3 rounded-2xl px-3 py-4") do
          header_row
          body_text
          action_bar
        end
      end

      private

      def header_row
        div(class: "flex items-start justify-between gap-2") do
          div(class: "flex min-w-0 items-start gap-2.5") do
            icon_circle
            div(class: "min-w-0") do
              p(class: "text-[12px] font-semibold uppercase tracking-wider text-muted-foreground") { t(".label") }
              p(class: "truncate text-sm font-semibold text-foreground") { safe_text(subject.title) }
            end
          end
          p(class: "flex-shrink-0 whitespace-nowrap text-[11px] text-muted-foreground") { relative_time(subject.created_at) }
        end
      end

      def body_text
        return if subject.body.blank?

        p(class: "text-sm leading-relaxed text-foreground/80") { safe_text(subject.body) }
      end

      def action_bar
        div(class: "flex items-center justify-end gap-2 pt-1") do
          dismiss_button(label: t(".later"), key: "x")
          act_button(tool: "notice_done", label: t(".done"), variant: :ghost, key: "e", primary: false)
          open_link
        end
      end

      # Open is the primary (→). Uses the notification's own link_url; when the
      # notice is unmistakable, a sharper verb than the generic "Open".
      def open_link
        return if subject.link_url.blank?

        link_button(href: subject.link_url, label: open_label, variant: :primary, key: "o")
      end

      def open_label
        key = subject.group_key.to_s
        return t(".reconnect") if key.start_with?("account_disconnected")
        return t(".review") if key.start_with?("document_review")

        t(".open")
      end

      def icon_circle
        span(class: "flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-muted text-foreground/70") do
          raw safe(<<~SVG)
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">#{category_icon}</svg>
          SVG
        end
      end

      def category_icon
        ICONS.fetch(subject.category&.to_sym, DEFAULT_ICON)
      end
    end
  end
end
