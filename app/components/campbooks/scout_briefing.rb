# frozen_string_literal: true

module Campbooks
  # The proactive landing for a fresh Scout chat. Instead of a blank "ask me
  # anything" box, Scout greets the user and surfaces what actually needs them
  # right now — live counts as one-tap prompts — then offers starter questions.
  #
  # All data is precomputed by the controller and passed in; this component is
  # pure presentation.
  class ScoutBriefing < Campbooks::Base
    # @param greeting [String] e.g. "Good evening, Alex"
    # @param subtitle [String] one line on what Scout can do / what's going on
    # @param stats [Array<Hash>] cards: { value:, label:, prompt:, icon:, tone: }
    # @param suggestions [Array<String>] starter prompt chips
    def initialize(greeting:, subtitle:, stats: [], suggestions: [])
      @greeting = greeting
      @subtitle = subtitle
      @stats = Array(stats).first(4)
      @suggestions = Array(suggestions)
    end

    def view_template
      div(id: "agent_empty_state", class: "flex items-center justify-center min-h-full px-5 py-10") do
        div(class: "w-full max-w-xl text-center animate-fade-in") do
          render Campbooks::ScoutAvatar.new(size: :xl, class: "mx-auto mb-4")
          h1(class: "text-xl font-semibold tracking-tight text-foreground") { @greeting }
          p(class: "mt-1.5 text-sm text-muted-foreground mx-auto max-w-md") { @subtitle }

          stat_grid if @stats.any?

          if @suggestions.any?
            div(class: "mt-7") do
              render Campbooks::ChatSuggestions.new(
                prompts: @suggestions,
                heading: t(".try_asking"),
                align: :center
              )
            end
          end
        end
      end
    end

    private

    def stat_grid
      # Avoid an orphan card: 4 stats lay out as a clean 2x2, 3 as 2-then-3.
      cols = @stats.length == 3 ? "grid-cols-2 sm:grid-cols-3" : "grid-cols-2"
      div(class: class_names("mt-7 grid gap-2.5", cols)) do
        @stats.each { |stat| stat_card(stat) }
      end
    end

    def stat_card(stat)
      tone = TONES[stat[:tone]&.to_sym] || TONES[:default]
      button(
        type: "button",
        data: { action: "chat-input#prompt", chat_input_text_param: stat[:prompt] },
        title: stat[:prompt],
        class: "group flex flex-col gap-3 rounded-xl border border-border bg-card p-4 text-left shadow-sm " \
               "transition-all duration-150 ease-out hover:-translate-y-0.5 hover:border-accent-300 " \
               "hover:shadow-md cursor-pointer"
      ) do
        div(class: "flex items-center justify-between") do
          span(class: class_names("flex h-8 w-8 items-center justify-center rounded-lg", tone[:chip])) do
            raw(safe(ICONS[stat[:icon]&.to_sym] || ICONS[:sparkles]))
          end
          raw(safe(ARROW))
        end
        div do
          div(class: class_names("text-2xl font-semibold leading-none tabular-nums", tone[:value])) { format_value(stat[:value]) }
          div(class: "mt-1 text-[11px] font-medium text-muted-foreground truncate") { stat[:label].to_s }
        end
      end
    end

    def format_value(value)
      value.is_a?(Numeric) ? helpers.number_with_delimiter(value) : value.to_s
    end

    TONES = {
      default: { chip: "bg-muted text-muted-foreground", value: "text-foreground" },
      accent:  { chip: "bg-accent-100 text-accent-700 dark:bg-accent-500/15 dark:text-accent-300", value: "text-accent-700 dark:text-accent-300" },
      amber:   { chip: "bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300", value: "text-amber-600 dark:text-amber-300" },
      green:   { chip: "bg-green-100 text-green-700 dark:bg-green-500/15 dark:text-green-300", value: "text-green-600 dark:text-green-300" }
    }.freeze

    ARROW = '<svg class="w-4 h-4 text-muted-foreground/40 transition-all group-hover:text-accent-500 group-hover:translate-x-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"/></svg>'

    ICONS = {
      inbox:    '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"/></svg>',
      flag:     '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 2H21l-3 6 3 6h-8.5l-1-2H5a2 2 0 00-2 2z"/></svg>',
      document: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>',
      users:    '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a4 4 0 00-3-3.87M9 20H4v-2a4 4 0 013-3.87m6-1.13a4 4 0 10-4-4 4 4 0 004 4z"/></svg>',
      clock:    '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>',
      sparkles: '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5z" clip-rule="evenodd"/></svg>'
    }.freeze
  end
end
