# frozen_string_literal: true

module Campbooks
  module Activity
    # How a workspace Event reads to a human: its label, its subject link, a
    # payload detail, and an icon. Extracted from Campbooks::Activity::EventRow so
    # Scout's log (Campbooks::Now::LogRow) tells the same story the Activity
    # timeline does, from one source. Mix into a Campbooks::Base component that
    # holds the event in @event; it uses the component's helpers/t at render time.
    module EventPresentation
      def definition
        @definition ||= Events::Registry.definition(@event.name)
      end

      def icon_key
        definition&.icon || :default
      end

      def icon_svg
        raw(safe(ICONS.fetch(icon_key, ICONS[:default])))
      end

      # i18n label (events.names.<key>) with the registry's English label as the
      # default, humanized name for unregistered event types.
      def label
        key = definition&.i18n_key || @event.name.tr(".", "_")
        fallback = definition&.label || @event.name.tr(".", " ").humanize
        t("events.names.#{key}", default: fallback)
      end

      # A single most-relevant payload value for the secondary line / log subject.
      def payload_summary
        payload = @event.payload || {}
        primary = payload.values_at("subject", "filename", "title", "name", "tag", "to", "email_address").compact.first
        return primary if primary.present?
        return t("events.bulk_count", count: payload["count"]) if payload["count"].present?

        nil
      end

      # Link to the subject's canonical page for record types with a stable show
      # route. Built from subject_type/id (no record load) — a deleted subject's
      # link 404s, which is acceptable and rare within the retention window.
      def subject_path
        return nil unless @event.subject_id

        case @event.subject_type
        when "Document" then helpers.document_path(@event.subject_id)
        when "Contact" then helpers.contact_path(@event.subject_id)
        when "MailFolder" then helpers.files_folder_path(@event.subject_id)
        when "AuthoredDocument" then helpers.written_document_path(@event.subject_id)
        when "EmailMessage" then helpers.email_message_path(@event.subject_id)
        when "DigestIssue" then digest_issue_path
        end
      end

      # A digest issue's page is nested under its digest, so unlike the flat routes
      # above this needs the parent id — read off the (preloaded, in Scout's log)
      # subject. Nil for a deleted issue, which drops the link rather than 500ing.
      def digest_issue_path
        issue = @event.subject
        return nil unless issue

        helpers.digest_issue_path(issue.scheduled_digest_id, issue.id)
      end

      ICONS = {
        mail: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>',
        archive: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="4" rx="1"/><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8M10 12h4"/></svg>',
        trash: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2m-9 0v14a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6"/></svg>',
        clock: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>',
        tag: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7v5l9 9 7-7-9-9H4z"/><circle cx="7.5" cy="7.5" r="1.2"/></svg>',
        send: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 3 10 14M21 3l-7 18-4-7-7-4z"/></svg>',
        inbox: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 13h4l2 3h6l2-3h4M5 5h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z"/></svg>',
        document: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/></svg>',
        check: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>',
        x: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18M6 6l12 12"/></svg>',
        calendar: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4.5" width="18" height="16" rx="2"/><path d="M3 9h18M8 3v4M16 3v4"/></svg>',
        star: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4l2.5 5 5.5.8-4 3.9 1 5.5L12 16.8 7.5 19.2l1-5.5-4-3.9 5.5-.8z"/></svg>',
        ban: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M5.6 5.6l12.8 12.8"/></svg>',
        bell: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 7 3 9H3c0-2 3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/></svg>',
        plug: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9 2v6M15 2v6M7 8h10v3a5 5 0 0 1-10 0zM12 16v6"/></svg>',
        folder: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>',
        check_square: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="m8 12 2.5 2.5L16 9"/></svg>',
        user: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>',
        default: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.2"/><circle cx="12" cy="12" r="9"/></svg>'
      }.freeze
    end
  end
end
