# frozen_string_literal: true

module Campbooks
  # Loading placeholder for the inbox search-results frame. The email-search
  # Stimulus controller reveals this (and hides the stale frame) on
  # `turbo:before-fetch-request`, then hides it again on `turbo:frame-render`, so
  # a slow semantic query reads as "working" instead of a frozen pane. The row
  # rhythm mirrors Campbooks::EmailSearchResult (avatar :lg + subject/sender/
  # snippet lines) to minimise the shift when real rows swap in.
  class EmailSearchSkeleton < Campbooks::Base
    def initialize(rows: 6, **attrs)
      @rows = rows
      @attrs = attrs
    end

    def view_template
      div(
        class: class_names("animate-skeleton", @attrs.delete(:class)),
        aria_hidden: "true",
        **@attrs
      ) do
        cue
        div(class: "py-1") do
          @rows.times { skeleton_row }
        end
      end
    end

    private

    # Mirrors the "Search results" heading band, but with a live spinner + label.
    def cue
      div(class: "flex items-center gap-2 px-2.5 py-1.5 border-b border-gray-100") do
        render(Campbooks::Spinner.new(size: :sm, class: "w-3 h-3"))
        span(class: "text-[10px] font-medium text-gray-500") { t(".searching") }
      end
    end

    def skeleton_row
      div(class: "flex items-start gap-2.5 mx-1.5 rounded-xl px-2.5 py-2") do
        div(class: "w-8 h-8 rounded-xl bg-gray-200 shrink-0 mt-px")
        div(class: "min-w-0 flex-1 pt-0.5 space-y-1.5") do
          div(class: "flex items-center justify-between gap-2") do
            div(class: "h-2.5 rounded bg-gray-200 w-1/2")
            div(class: "h-2 rounded bg-gray-200 w-6")
          end
          div(class: "h-2 rounded bg-gray-200 w-1/3")
          div(class: "h-2 rounded bg-gray-200 w-4/5")
        end
      end
    end
  end
end
