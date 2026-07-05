# frozen_string_literal: true

module Campbooks
  module Feed
    # Home-feed card for a generated DigestIssue. Shows the digest name,
    # overview (or item count when overview is absent/list-mode), up to 3
    # item titles, and a CTA to the full issue page.
    # Left-swipe dismisses (generic feed dismiss — same pattern as task card).
    class DigestCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex flex-col gap-3 rounded-2xl px-3 py-4 transition-colors duration-150 hover:bg-muted/50") do
          header_row
          body_section
          action_bar
        end
      end

      private

      def header_row
        div(class: "flex items-center justify-between gap-2") do
          div(class: "flex items-center gap-2") do
            icon_circle
            div do
              p(class: "text-[12px] font-semibold uppercase tracking-wider text-muted-foreground") { t(".label") }
              p(class: "truncate text-sm font-semibold text-foreground") { digest_name }
            end
          end
          p(class: "flex-shrink-0 text-[11px] text-muted-foreground") { relative_time(subject.created_at) }
        end
      end

      def body_section
        if overview.present?
          p(class: "line-clamp-2 text-sm leading-relaxed text-foreground/80") { overview }
        else
          p(class: "text-[13px] text-muted-foreground") { t(".item_count", count: item_count) }
        end

        titles = first_item_titles
        if titles.any?
          div(class: "mt-2 space-y-0.5") do
            titles.each do |title|
              p(class: "truncate text-[12.5px] text-muted-foreground before:mr-1.5 before:content-['·']") { title }
            end
          end
        end
      end

      def action_bar
        div(class: "flex items-center justify-end gap-2 pt-1") do
          dismiss_button(label: t(".not_now"), key: "x")
          link_button(href: issue_path, label: t(".view_digest"), variant: :primary, key: "o")
        end
      end

      def digest_name
        item.data["digest_name"].to_s.presence || t(".digest")
      end

      def overview
        (item.data["overview"].presence || subject.overview).to_s.presence
      end

      def item_count
        item.data["item_count"].to_i
      end

      def first_item_titles
        subject.sections
               .flat_map { |s| Array(s["items"]).map { |i| i["title"].to_s.presence }.compact }
               .first(3)
      rescue StandardError
        []
      end

      def issue_path
        helpers.digest_issue_path(subject.scheduled_digest_id, subject.id)
      rescue StandardError
        helpers.digests_path
      end

      def icon_circle
        span(class: "flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-full bg-muted text-foreground/70") do
          raw safe('<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11L2 12v6a2 2 0 002 2h16a2 2 0 002-2v-6l-3.45-6.89A2 2 0 0016.76 4H7.24a2 2 0 00-1.79 1.11z"/></svg>')
        end
      end
    end
  end
end
