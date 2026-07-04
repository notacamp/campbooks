module Campbooks
  module Feed
    # An AI-detected reminder awaiting confirmation: type, due date, optional amount
    # or context, and two actions — "Add to calendar" (confirm) and "Dismiss". Both
    # go through Feed::ItemsController#act, which updates the Reminder and removes the
    # card. Borderless nudge styling like CalendarEventCard. `subject` is a Reminder.
    class ReminderCard < Campbooks::Feed::Base
      def view_template
        div(class: "-mx-3 flex items-start gap-3 rounded-2xl px-3 py-3 transition-colors duration-150 hover:bg-muted/50") do
          icon_circle
          div(class: "min-w-0 flex-1") do
            div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
              span(class: "font-medium text-foreground") { type_label }
              span(class: "text-muted-foreground/50") { "·" }
              span(class: class_names("font-medium", overdue? ? "text-red-600" : "text-muted-foreground")) { due_text }
            end
            div(class: "mt-1 truncate text-sm font-semibold leading-snug text-foreground") { subject.title }
            p(class: "mt-0.5 text-[13px] text-muted-foreground") { amount } if amount
            if subject.justification.present?
              p(class: "mt-0.5 line-clamp-1 text-[12.5px] italic text-muted-foreground") { subject.justification }
            elsif subject.description.present?
              p(class: "mt-0.5 line-clamp-1 text-[13px] text-muted-foreground") { subject.description }
            end
            render Campbooks::ReminderSourceLinks.new(reminder: subject, css: "mt-1.5")
            # Confirming a date usually means double-checking the email that
            # carried it — show it in place instead of sending the user away.
            if subject.source_email
              render Campbooks::Feed::ExpandablePreview.new(
                item: item, label: t("components.feed.expandable_preview.show_source"), class: "mt-1.5"
              )
            end
            div(class: "mt-2.5 flex items-center justify-end gap-2") do
              act_button(tool: "dismiss_reminder", label: t(".dismiss"), variant: :ghost, key: "x", dismiss: true)
              act_button(tool: "confirm", label: t(".confirm"), variant: :primary, key: "c", primary: true)
            end
          end
        end
      end

      private

      def overdue? = subject.due_at < Time.current

      def type_label = helpers.human_enum(::Reminder, :reminder_type, subject.reminder_type)

      def amount
        return nil if subject.amount_cents.blank?
        subject.money&.format
      end

      # "Overdue" / "Today" / "Tomorrow" / "In N days" / a date — with the clock time
      # appended for timed reminders.
      def due_text
        days = (subject.due_at.to_date - Date.current).to_i
        label =
          if days.negative? then t(".overdue")
          elsif days.zero? then t(".today")
          elsif days == 1 then t(".tomorrow")
          elsif days <= 14 then t(".in_days", count: days)
          else l(subject.due_at.to_date, format: :long)
          end
        subject.all_day? ? label : "#{label} · #{l(subject.due_at, format: :clock)}"
      end

      def icon_circle
        span(class: "mt-0.5 flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-ember/10 text-ember") do
          raw safe(spark_icon)
        end
      end

      def spark_icon
        %(<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>)
      end
    end
  end
end
