# frozen_string_literal: true

module Campbooks
  # The inbox search bar that sits atop the thread-list pane. A GET <form> that
  # navigates the `email_search_results` Turbo Frame in place (so the folder
  # sidebar + reading pane stay put). Holds the text query, the keyword/meaning
  # toggle, and a Filters button that reveals the EmailFilterPanel. Active-filter
  # chips render inside the results frame (so they refresh with the results).
  class EmailSearchBar < Campbooks::Base
    SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'
    CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'
    FILTER_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4.5h18M6.75 9.75h10.5M10.5 15h3"/>'

    def initialize(search_params: {}, folders: [], accounts: [], tags: [], **attrs)
      @search_params = normalize(search_params)
      @folders = folders
      @accounts = accounts
      @tags = tags
      @attrs = attrs
    end

    def view_template
      form(
        method: "get",
        action: helpers.search_email_messages_path,
        class: class_names("border-b border-gray-100", @attrs.delete(:class)),
        data: {
          controller: "email-search",
          turbo_frame: "email_search_results",
          # replace (not advance) so live typing updates the URL without spamming
          # one history entry per keystroke.
          turbo_action: "replace",
          action: "change->email-search#submitNow"
        },
        **@attrs
      ) do
        input_row
        panel
      end
    end

    private

    def input_row
      div(class: "flex items-center gap-1.5 px-2.5 py-1.5") do
        icon(SEARCH_ICON, "w-3.5 h-3.5 text-gray-400 flex-shrink-0")
        div(class: "flex-1 min-w-0") do
          input(
            type: "text",
            name: "q",
            value: @search_params[:q],
            placeholder: t(".placeholder"),
            autocomplete: "off",
            class: "block w-full bg-transparent border-0 p-0 text-xs text-gray-700 dark:text-gray-200 placeholder:text-gray-400 focus:ring-0 focus:outline-none",
            data: { email_search_target: "query", action: "input->email-search#scheduleSubmit keydown->email-search#handleKeydown" }
          )
        end
        clear_link if @search_params[:q].present?
        filters_button
      end
    end

    def clear_link
      a(
        href: helpers.email_messages_path,
        title: t(".clear"),
        aria_label: t(".clear"),
        class: "flex items-center justify-center w-5 h-5 rounded text-gray-400 hover:text-gray-600 hover:bg-gray-100 flex-shrink-0",
        data: { turbo_frame: "_top" }
      ) { icon(CLEAR_ICON, "w-3 h-3") }
    end

    def filters_button
      render(Campbooks::Button.new(
        variant: :ghost, size: :xs, type: :button,
        class: "flex-shrink-0 gap-1 text-gray-500 hover:text-gray-700",
        data: { action: "click->email-search#toggleFilters" }
      )) do
        icon(FILTER_ICON, "w-3.5 h-3.5")
        span(class: "hidden sm:inline") { t(".filters") }
        render(Campbooks::Badge.new(variant: :accent, size: :sm)) { active_count.to_s } if active_count.positive?
      end
    end

    def panel
      div(class: "border-t border-gray-100 px-2.5 py-2.5 hidden", data: { email_search_target: "filterPanel" }) do
        render(Campbooks::EmailFilterPanel.new(folders: @folders, accounts: @accounts, tags: @tags, active: @search_params))
      end
    end

    def icon(path, classes)
      svg(class: classes, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(path)) }
    end

    def active_count
      count = 0
      count += 1 if @search_params[:folder].present? && @search_params[:folder] != "all"
      count += Array(@search_params[:account_ids]).reject(&:blank?).size
      count += Array(@search_params[:tag_ids]).reject(&:blank?).size
      %i[sender domain date_from date_to category priority].each { |k| count += 1 if @search_params[k].present? }
      count += 1 if @search_params[:has_attachment].to_s == "1"
      count += 1 if @search_params[:unread].to_s == "1"
      count
    end

    def normalize(params)
      return {} if params.nil?
      (params.respond_to?(:to_unsafe_h) ? params.to_unsafe_h : params).symbolize_keys
    end
  end
end
