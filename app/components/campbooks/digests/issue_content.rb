# frozen_string_literal: true

module Campbooks
  module Digests
    # Renders the content of a DigestIssue: overview + sections + items.
    # Shared by the digest show page (inline latest issue) and the issue page.
    # Resolves section keys to i18n titles; links items to their source pages
    # via path helpers (web UI uses path-style links, not full URLs).
    class IssueContent < Campbooks::Base
      SOURCE_TYPE_ICONS = {
        "email"          => '<path d="M3 7a2 2 0 012-2h14a2 2 0 012 2v10a2 2 0 01-2 2H5a2 2 0 01-2-2V7z"/><path d="m3 7 9 6 9-6"/>',
        "calendar_event" => '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>',
        "task"           => '<path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2"/><path d="M9 5a2 2 0 012-2h2a2 2 0 012 2"/><path d="m9 14 2 2 4-4"/>',
        "reminder"       => '<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>',
        "document"       => '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/>'
      }.freeze

      def initialize(issue:)
        @issue = issue
      end

      def view_template
        article(class: "space-y-6") do
          overview_block if @issue.overview.present?
          @issue.sections.each { |sec| render_section(sec) }
          empty_notice if @issue.sections.empty?
        end
      end

      private

      def overview_block
        div(class: "rounded-xl border border-border bg-muted/30 px-4 py-3 text-sm leading-relaxed text-foreground") do
          p { @issue.overview }
        end
      end

      def render_section(section)
        items = Array(section["items"])
        return if items.empty?

        div(class: "space-y-2") do
          h3(class: "text-sm font-semibold uppercase tracking-wide text-muted-foreground") { section_title(section) }
          div(class: "space-y-1") { items.each { |item| render_item(item) } }
        end
      end

      def section_title(section)
        return section["title"].to_s if section["title"].present?

        key = section["key"].to_s
        helpers.t("digests.sections.#{key}", default: key.humanize)
      end

      def render_item(item_hash)
        source_type = item_hash["source_type"].to_s
        source_id   = item_hash["source_id"].to_s
        title_text  = item_hash["title"].to_s.presence || t(".untitled")
        subtitle    = item_hash["subtitle"].to_s
        note        = item_hash["note"].to_s.presence
        timestamp   = item_hash["timestamp"].to_s

        url = item_path(source_type, source_id)

        div(class: "-mx-1 flex items-start gap-2.5 rounded-lg px-2 py-2 transition-colors hover:bg-muted/60") do
          source_icon(source_type)
          div(class: "min-w-0 flex-1") do
            a(href: url, class: "block truncate text-sm font-medium text-foreground hover:underline") { title_text }
            p(class: "mt-0.5 truncate text-[12px] text-muted-foreground") { subtitle } if subtitle.present?
            p(class: "mt-0.5 text-[12px] italic text-muted-foreground") { note } if note
          end
          if timestamp.present?
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground") { format_timestamp(timestamp) }
          end
        end
      end

      def source_icon(source_type)
        path = SOURCE_TYPE_ICONS[source_type] || SOURCE_TYPE_ICONS["document"]
        span(class: "mt-0.5 flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
          raw safe(%(<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">#{path}</svg>))
        end
      end

      def item_path(source_type, source_id)
        case source_type
        when "email"          then helpers.email_message_path(source_id)
        when "calendar_event" then helpers.calendar_path
        when "task"           then helpers.tasks_path
        when "reminder"       then helpers.reminders_path
        when "document"       then helpers.document_path(source_id)
        else "/"
        end
      rescue StandardError
        "/"
      end

      def format_timestamp(iso)
        time = Time.iso8601(iso)
        l(time.to_date == Date.current ? time : time.to_date, format: :short)
      rescue ArgumentError, TypeError
        ""
      end

      def empty_notice
        p(class: "py-4 text-center text-sm text-muted-foreground") { t(".empty") }
      end
    end
  end
end
