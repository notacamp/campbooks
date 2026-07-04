module Campbooks
  module Feed
    # A task on the home feed, in the two flavors of Feed::Sources::Task.
    #
    # SUGGESTED (AI-extracted, awaiting triage): "Suggested task" framing with the
    # extractor's justification, and Dismiss / Add-to-tasks actions that resolve
    # the suggestion itself (mirrors ReminderCard's confirm/dismiss pair).
    #
    # ACTIVE (needs attention): status pill, due text, title (links to the task)
    # and a Complete action — plus the generic dismiss to clear the card without
    # changing the task. `subject` is a Task.
    class TaskCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            subject.suggested? ? suggested_body : active_body
          end
        end
      end

      private

      def suggested_body
        div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
          span(class: "font-medium text-foreground") { t(".suggested") }
          if subject.due_at
            span(class: "text-muted-foreground/50") { "·" }
            span(class: "font-medium text-muted-foreground") { due_text }
          end
        end
        a(href: helpers.task_path(subject),
          class: "mt-1 block truncate text-sm font-semibold leading-snug text-foreground hover:underline") { subject.title }
        if subject.justification.present?
          p(class: "mt-0.5 line-clamp-1 text-[12.5px] italic text-muted-foreground") { subject.justification }
        elsif subject.description.present?
          p(class: "mt-0.5 line-clamp-1 text-[13px] text-muted-foreground") { subject.description }
        end
        # Accept/dismiss is a judgement on the email the task came from — show
        # that source in place instead of sending the user away.
        source_email_preview
        div(class: "mt-2.5 flex items-center justify-end gap-2") do
          act_button(tool: "dismiss_task", label: t(".dismiss"), variant: :ghost, key: "x", dismiss: true)
          act_button(tool: "accept", label: t(".accept"), variant: :primary, key: "c", primary: true)
        end
      end

      def active_body
        div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
          render Campbooks::Tasks::StatusBadge.new(status: subject.status, size: :sm)
          if subject.due_at
            span(class: "text-muted-foreground/50") { "·" }
            span(class: class_names("font-medium", subject.overdue? ? "text-red-600" : "text-muted-foreground")) { due_text }
          end
          if subject.recurring?
            span(class: "text-muted-foreground/50") { "·" }
            render Campbooks::RecurrenceIcon.new(css: "w-3 h-3 text-muted-foreground shrink-0")
          end
        end
        a(href: helpers.task_path(subject),
          class: "mt-1 block truncate text-sm font-semibold leading-snug text-foreground hover:underline") { subject.title }
        if subject.assignees.any?
          p(class: "mt-0.5 truncate text-[12.5px] text-muted-foreground") { subject.assignees.first(3).map(&:name).join(", ") }
        end
        source_email_preview
        div(class: "mt-2.5 flex items-center justify-end gap-2") do
          dismiss_button(label: t(".dismiss"), key: "x")
          act_button(tool: "complete", label: t(".complete"), variant: :primary, key: "c", primary: true)
        end
      end

      def source_email_preview
        return unless subject.source_email

        render Campbooks::Feed::ExpandablePreview.new(
          item: item, label: t("components.feed.expandable_preview.show_source"), class: "mt-1.5"
        )
      end

      def due_text
        days = (subject.due_at.to_date - Date.current).to_i
        if days.negative? then t(".overdue")
        elsif days.zero? then t(".today")
        elsif days == 1 then t(".tomorrow")
        elsif days <= 14 then t(".in_days", count: days)
        else l(subject.due_at.to_date, format: :full)
        end
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-ember/10 text-ember") do
          raw safe(check_icon)
        end
      end

      def check_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M9 11l3 3L20 6"/></svg>)
      end
    end
  end
end
