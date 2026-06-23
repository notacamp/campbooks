module Campbooks
  # A reminder on the /reminders page. Flat on the canvas (no boxed card) with a
  # hover fill that bleeds to the gutter — the Linear/Notion list-row pattern from
  # DESIGN.md. An Ember Scout mark signals "Scout found this"; the action bar is
  # right-aligned with the primary (Add to calendar) on the far right. Date editing
  # is progressive: hidden behind an "Adjust date" disclosure so the row stays calm,
  # while one tap on "Add to calendar" uses the extracted date. Posts to
  # RemindersController (Turbo-stream removes the row).
  class ReminderRow < Campbooks::Base
    def initialize(reminder:)
      @reminder = reminder
    end

    def view_template
      div(
        id: "reminder_#{@reminder.id}",
        class: class_names(
          "-mx-3 rounded-xl px-3 py-3 transition-colors hover:bg-muted/40",
          @reminder.confirmed? ? "opacity-60" : nil
        )
      ) do
        div(class: "flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between") do
          div(class: "flex min-w-0 items-start gap-3") do
            leading_mark
            div(class: "min-w-0 flex-1") { body }
          end
          @reminder.confirmed? ? confirmed_actions : action_bar
        end
      end
    end

    private

    def body
      meta_line
      div(class: "mt-0.5 font-semibold leading-snug text-foreground") { @reminder.title }
      p(class: "mt-0.5 text-sm text-muted-foreground") { amount } if amount
      p(class: "mt-1 text-[13px] leading-snug text-muted-foreground") { @reminder.justification } if @reminder.justification.present?
      render Campbooks::ReminderSourceLinks.new(reminder: @reminder, css: "mt-1.5")
      date_disclosure unless @reminder.confirmed?
    end

    def meta_line
      div(class: "flex flex-wrap items-center gap-x-1.5 text-[12.5px]") do
        span(class: "font-medium text-foreground") { type_label }
        span(class: "text-muted-foreground/50") { "·" }
        span(class: class_names("font-medium", overdue? ? "text-red-600" : "text-muted-foreground")) { due_text }
      end
    end

    # Right-aligned bar: priority increases right-to-left, primary on the far right.
    def action_bar
      div(class: "flex shrink-0 items-center gap-1.5 sm:pt-0.5") do
        post_button(helpers.dismiss_reminder_path(@reminder), t(".dismiss"))
        post_button(helpers.snooze_reminder_path(@reminder), t(".snooze"))
        confirm_form
      end
    end

    def confirmed_actions
      div(class: "flex shrink-0 items-center gap-2 sm:pt-0.5") do
        if @reminder.calendar_event_id
          render Campbooks::Button.new(variant: :ghost, size: :sm, href: helpers.edit_calendar_event_path(@reminder.calendar_event_id)) { t(".view_event") }
        else
          span(class: "text-sm text-muted-foreground") { t(".confirmed") }
        end
      end
    end

    # The date editor lives here but belongs to the confirm form via the `form=`
    # attribute, so one "Add to calendar" submit carries the (possibly edited) date.
    def date_disclosure
      details(class: "mt-2 text-[13px]") do
        summary(class: "inline-flex w-fit cursor-pointer items-center gap-1 text-muted-foreground hover:text-foreground") { t(".adjust_date") }
        input(
          type: "datetime-local", name: "due_at", form: confirm_form_id,
          value: @reminder.due_at.strftime("%Y-%m-%dT%H:%M"),
          class: "mt-2 block rounded-lg border border-border bg-background px-2 py-1.5 text-sm"
        )
      end
    end

    def confirm_form
      form(id: confirm_form_id, action: helpers.confirm_reminder_path(@reminder), method: :post, class: "inline-flex") do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        render Campbooks::Button.new(variant: :primary, size: :sm, type: "submit") { t(".confirm") }
      end
    end

    def post_button(url, label)
      form(action: url, method: :post, class: "inline-flex") do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        render Campbooks::Button.new(variant: :ghost, size: :sm, type: "submit") { label }
      end
    end

    def confirm_form_id = "reminder_confirm_#{@reminder.id}"

    # Ember Scout spark: marks the row as something Scout surfaced (the Meaning Rule —
    # Ember = Scout / this wants you).
    def leading_mark
      span(class: "mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-ember/10 text-ember") do
        raw safe(spark_icon)
      end
    end

    def spark_icon
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>)
    end

    def overdue? = @reminder.pending? && @reminder.due_at < Time.current

    def type_label = helpers.human_enum(::Reminder, :reminder_type, @reminder.reminder_type)

    def amount
      return nil if @reminder.amount_cents.blank?
      @reminder.money&.format
    end

    def due_text
      days = (@reminder.due_at.to_date - Date.current).to_i
      label =
        if days.negative? then t(".overdue")
        elsif days.zero? then t(".today")
        elsif days == 1 then t(".tomorrow")
        elsif days <= 14 then t(".in_days", count: days)
        else l(@reminder.due_at.to_date, format: :long)
        end
      @reminder.all_day? ? label : "#{label} · #{l(@reminder.due_at, format: :clock)}"
    end
  end
end
