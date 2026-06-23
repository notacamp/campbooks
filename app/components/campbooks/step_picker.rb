module Campbooks
  # Zapier/n8n-style "add a step" picker: a modal with a search box over a list
  # of selectable cards (one per step type). Hidden by default and opened
  # client-side by the `step-picker` Stimulus controller, so it feels instant.
  #
  # Rendered once per page (outside the main workflow <form>, since each card is
  # its own POST form). Every "+" connector opens this same instance.
  class StepPicker < Campbooks::Base
    # The logic/condition card is defined here; the action cards come from the
    # single Workflows::ActionRegistry, so the picker and the executor can never
    # drift apart. `group` drives the heading + accent colour.
    CONDITION_CARD_TEMPLATE = {
      group: :logic, key: "condition", icon: :filter,
      step_type: "condition"
    }.freeze

    CATALOG_BASE = ([ CONDITION_CARD_TEMPLATE ] + Workflows::ActionRegistry.picker_cards).freeze

    GROUP_ACCENT = {
      logic: "tone-amber",
      action: "tone-green"
    }.freeze

    def initialize(workflow:, open: false, **attrs)
      @workflow = workflow
      @open = open
      @attrs = attrs
    end

    def view_template
      div(
        class: class_names(
          ("hidden" unless @open),
          "fixed inset-0 z-50 flex items-start justify-center p-4 pt-[10vh] bg-black/50 backdrop-blur-sm"
        ),
        data: { step_picker_target: "modal", action: "click->step-picker#backdropClose keydown->step-picker#keydown" },
        role: "dialog", aria_modal: "true", aria_label: t(".dialog_label")
      ) do
        div(class: "bg-card text-card-foreground rounded-xl shadow-xl border border-border w-full max-w-lg max-h-[80vh] flex flex-col overflow-hidden") do
          render_search
          render_list
        end
      end
    end

    private

    def render_search
      div(class: "flex items-center gap-2 px-4 py-3 border-b border-border") do
        span(class: "text-muted-foreground flex-shrink-0") { icon(:search) }
        input(
          type: "text",
          placeholder: t(".search_placeholder"),
          autocomplete: "off",
          class: "flex-1 min-w-0 bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none",
          data: { step_picker_target: "search", action: "input->step-picker#filter" }
        )
        button(
          type: "button",
          class: "text-muted-foreground hover:text-foreground flex-shrink-0 cursor-pointer",
          aria_label: t("shared.actions.close"),
          data: { action: "click->step-picker#close" }
        ) { icon(:x) }
      end
    end

    def render_list
      div(class: "flex-1 overflow-y-auto p-2", data: { step_picker_target: "list" }) do
        catalog.group_by { |i| i[:group] }.each do |group, items|
          div(data: { step_picker_target: "group" }) do
            div(class: "px-2 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground") { group_labels[group] }
            items.each { |item| render_card(item) }
          end
        end

        div(
          class: "hidden px-3 py-8 text-center text-sm text-muted-foreground",
          data: { step_picker_target: "empty" }
        ) { t(".no_matching_steps") }
      end
    end

    def render_card(item)
      form(action: add_url(item), method: "post", class: "block") do
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
        button(
          type: "submit",
          class: "group/card w-full flex items-start gap-3 text-left px-2 py-2 rounded-lg hover:bg-accent data-[active=true]:bg-accent cursor-pointer transition-colors",
          data: {
            step_picker_target: "card",
            keywords: "#{item[:title]} #{item[:description]} #{item[:key]} #{group_labels[item[:group]]}".downcase,
            action: "mouseenter->step-picker#activate"
          }
        ) do
          span(class: class_names("w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0", GROUP_ACCENT[item[:group]])) { icon(item[:icon]) }
          span(class: "min-w-0") do
            span(class: "block text-sm font-medium text-foreground") { item[:title] }
            span(class: "block text-xs text-muted-foreground") { item[:description] }
          end
        end
      end
    end

    def catalog
      @catalog ||= CATALOG_BASE.map do |item|
        if item[:key] == "condition"
          item.merge(title: t(".condition_title"), description: t(".condition_description"))
        else
          item
        end
      end
    end

    def group_labels
      @group_labels ||= { logic: t(".group_labels.logic"), action: t(".group_labels.action") }
    end

    def add_url(item)
      params = { step_type: item[:step_type] }
      params[:action_type] = item[:action_type] if item[:action_type]
      helpers.add_step_workflow_path(@workflow, params)
    end

    def icon(name)
      raw safe(ICONS.fetch(name, ICONS[:bolt]))
    end

    ICONS = {
      search: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>',
      x: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>',
      filter: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2a1 1 0 01-.293.707L14 13.414V19a1 1 0 01-.553.894l-4 2A1 1 0 018 21v-7.586L3.293 6.707A1 1 0 013 6V4z"/></svg>',
      mail: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>',
      bolt: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>',
      chat: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"/></svg>',
      hash: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14"/></svg>',
      link: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.19 8.688a4.5 4.5 0 011.242 7.244l-4.5 4.5a4.5 4.5 0 01-6.364-6.364l1.757-1.757m13.35-.622l1.757-1.757a4.5 4.5 0 00-6.364-6.364l-4.5 4.5a4.5 4.5 0 001.242 7.244"/></svg>',
      inbox: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"/></svg>',
      folder: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>',
      upload: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2M12 4v12m0-12l-4 4m4-4l4 4"/></svg>',
      document: '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>'
    }.freeze
  end
end
