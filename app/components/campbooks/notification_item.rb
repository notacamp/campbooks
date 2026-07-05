# frozen_string_literal: true

module Campbooks
  # A single notification row, shared by the bell dropdown (compact) and the
  # notifications index (full). Priority tier is conveyed by the leading
  # category icon's tint and by the section it sits in — never a side-stripe.
  class NotificationItem < Campbooks::Base
    # @param notification [Notification]
    # @param compact [Boolean] tighter layout for the bell dropdown
    def initialize(notification:, compact: false)
      @n = notification
      @compact = compact
    end

    def view_template
      div(id: dom_identifier, class: row_classes) do
        div(class: class_names("flex-shrink-0 flex items-center justify-center rounded-full", @compact ? "w-7 h-7 mt-0.5" : "w-9 h-9", tier_icon_classes)) do
          raw(safe(icon_svg))
        end

        a(href: helpers.notification_path(@n), class: "flex-1 min-w-0 block") do
          div(class: "flex items-center gap-1.5") do
            unless @n.read?
              span(class: "h-1.5 w-1.5 flex-shrink-0 rounded-full bg-accent-500")
            end
            p(class: class_names(@compact ? "text-xs" : "text-sm", "font-medium truncate", @n.read? ? "text-muted-foreground" : "text-foreground")) { @n.title }
            if @n.count > 1
              render Campbooks::Badge.new(variant: :neutral, size: :sm) { @n.count.to_s }
            end
          end

          if @n.body.present?
            p(class: class_names(@compact ? "text-xs" : "text-sm", "text-muted-foreground mt-0.5", @compact ? "line-clamp-1" : "line-clamp-2")) { @n.body }
          end

          p(class: "text-[11px] text-muted-foreground/70 mt-1") { t(".time_ago", time: helpers.time_ago_in_words(@n.created_at)) }
        end

        if @n.archived?
          a(href: helpers.unarchive_notification_path(@n), data: { turbo_method: :post },
            class: trailing_classes, title: t(".restore_title"), aria: { label: t(".restore_aria_label") }) do
            raw(safe(%(<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 010 12h-3"/></svg>)))
          end
        else
          a(href: helpers.archive_notification_path(@n), data: { turbo_method: :post },
            class: trailing_classes, title: t(".archive_title"), aria: { label: t(".archive_aria_label") }) do
            raw(safe(%(<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12"/></svg>)))
          end
        end
      end
    end

    private

    # Distinct id per surface so a Turbo `remove` only hits the intended copy:
    # the same notification can appear in the bell dropdown, the index list and
    # a toast at once.
    def dom_identifier
      @compact ? "bell_notification_#{@n.id}" : "notification_#{@n.id}"
    end

    def trailing_classes
      "flex-shrink-0 self-start flex items-center justify-center w-6 h-6 rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted transition-colors"
    end

    def row_classes
      class_names("flex items-start gap-2.5 transition-colors hover:bg-muted/40", @compact ? "px-3 py-2.5" : "px-4 py-3.5")
    end

    def tier_icon_classes
      case @n.priority.to_sym
      when :action_required then "bg-destructive/10 text-destructive"
      when :awaiting        then "bg-accent-50 text-accent-600"
      else                       "bg-muted text-muted-foreground"
      end
    end

    def icon_svg
      size = @compact ? "w-3.5 h-3.5" : "w-4 h-4"
      %(<svg class="#{size}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5"><path stroke-linecap="round" stroke-linejoin="round" d="#{CATEGORY_ICONS[@n.category.to_sym] || CATEGORY_ICONS[:activity]}"/></svg>)
    end

    CATEGORY_ICONS = {
      system:   "M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z",
      document: "M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z",
      activity: "M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5",
      export:   "M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-3L12 18m0 0l4.5-4.5M12 18V4.5",
      ai_reply: "M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z",
      contact:  "M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z",
      mention:  "M16.5 12a4.5 4.5 0 11-9 0 4.5 4.5 0 019 0zm0 0c0 1.657 1.007 3 2.25 3S21 13.657 21 12a9 9 0 10-2.636 6.364",
      comment:  "M2.25 12.76c0 1.6 1.123 2.994 2.707 3.227 1.087.16 2.185.283 3.293.369V21l4.076-4.076a1.526 1.526 0 011.037-.443 48.282 48.282 0 005.68-.494c1.584-.233 2.707-1.626 2.707-3.228V6.741c0-1.602-1.123-2.995-2.707-3.228A48.394 48.394 0 0012 3c-2.392 0-4.744.175-7.043.513C3.373 3.746 2.25 5.14 2.25 6.741v6.018z",
      task:     "M11.35 3.836c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 00.75-.75 2.25 2.25 0 00-.1-.664m-5.8 0A2.251 2.251 0 0113.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m8.9-4.414c.376.023.75.05 1.124.08 1.131.094 1.976 1.057 1.976 2.192V16.5A2.25 2.25 0 0118 18.75h-2.25m-7.5-10.5H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V18.75m-7.5-10.5h6.375c.621 0 1.125.504 1.125 1.125v9.375m-8.25-3l1.5 1.5 3-3.75"
    }.freeze
  end
end
