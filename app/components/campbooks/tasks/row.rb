# frozen_string_literal: true

module Campbooks
  module Tasks
    # One task in the list view: status pill, title, due chip, a priority flag for
    # high/urgent work, label dots and assignee avatars. The whole row links to
    # the task detail.
    class Row < Campbooks::Base
      def initialize(task:)
        @task = task
      end

      def view_template
        a(href: helpers.task_path(@task),
          class: "group flex items-center gap-3 px-1 py-3 transition hover:bg-muted/50") do
          render Campbooks::Tasks::StatusBadge.new(status: @task.status)

          div(class: "min-w-0 flex-1") do
            div(class: "flex items-center gap-2") do
              span(class: class_names(
                "truncate text-sm font-medium",
                done_or_cancelled? ? "text-muted-foreground line-through" : "text-foreground"
              )) { @task.title }
              render Campbooks::RecurrenceIcon.new if @task.recurring?
              priority_flag if @task.priority_high? || @task.priority_urgent?
            end
            div(class: "mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground") do
              due_chip
              labels
            end
          end

          assignees
        end
      end

      private

      def done_or_cancelled?
        @task.done? || @task.cancelled?
      end

      def due_chip
        return span(class: "text-muted-foreground/70") { t(".no_due_date") } if @task.due_at.blank?

        span(class: class_names("inline-flex items-center", @task.overdue? ? "font-medium text-red-600 dark:text-red-400" : nil)) do
          plain(due_label)
        end
      end

      def due_label
        today = Date.current
        due = @task.due_at.to_date
        if @task.overdue?
          t(".overdue")
        elsif due == today
          t(".due_today")
        elsif due == today + 1
          t(".due_tomorrow")
        else
          t(".in_days", count: (due - today).to_i)
        end
      end

      def priority_flag
        span(class: class_names(
          "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
          @task.priority_urgent? ? "bg-red-500/10 text-red-600 dark:text-red-400" : "bg-amber-500/10 text-amber-700 dark:text-amber-300"
        )) { t("activerecord.attributes.task.priorities.#{@task.priority}") }
      end

      def labels
        @task.tags.first(3).each do |tag|
          span(class: "inline-flex items-center gap-1") do
            span(class: "inline-block h-2 w-2 rounded-full", style: "background-color: #{tag.color}")
            plain(tag.name)
          end
        end
      end

      def assignees
        members = @task.assignees.first(3)
        return if members.empty?

        div(class: "flex shrink-0 -space-x-1.5") do
          members.each { |u| render Campbooks::Avatar.new(name: u.name, size: :sm) }
        end
      end
    end
  end
end
