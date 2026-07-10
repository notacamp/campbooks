# frozen_string_literal: true

module Campbooks
  # The inbox search bar that sits atop the thread-list pane. A GET <form> that
  # navigates the `email_search_results` Turbo Frame in place (so the folder
  # sidebar + reading pane stay put). Holds the text query, the keyword/meaning
  # toggle, and a Filters button that reveals the EmailFilterPanel. Active-filter
  # chips render inside the results frame (so they refresh with the results).
  #
  # The suggestions panel renders modifier-typeahead rows driven by the
  # email-search Stimulus controller. The catalog is passed as a JSON value so
  # the JS can build its own DOM without round-trips.
  class EmailSearchBar < Campbooks::Base
    SEARCH_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"/>'.freeze
    CLEAR_ICON  = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'.freeze
    FILTER_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4.5h18M6.75 9.75h10.5M10.5 15h3"/>'.freeze

    def initialize(search_params: {}, folders: [], accounts: [], tags: [], **attrs)
      @search_params = normalize(search_params)
      @folders  = folders
      @accounts = accounts
      @tags     = tags
      @attrs    = attrs
    end

    def view_template
      form(
        method: "get",
        action: helpers.search_email_messages_path,
        class: class_names("border-b border-gray-100 relative", @attrs.delete(:class)),
        data: {
          controller: "email-search",
          turbo_frame: "email_search_results",
          # replace (not advance) so live typing updates the URL without spamming
          # one history entry per keystroke.
          turbo_action: "replace",
          action: "change->email-search#submitNow",
          email_search_suggestions_value: catalog.to_json,
          email_search_inbox_url_value: helpers.email_messages_path,
          email_search_heading_text: t(".suggest.heading"),
          email_search_heading_text_value: t(".suggest.heading")
        },
        **@attrs
      ) do
        div(class: "relative") do
          input_row
          progress_bar
        end
        suggestions_panel
        panel
      end
    end

    private

    # Indeterminate scan bar pinned to the base of the search field. Hidden until
    # the email-search controller reveals it on `turbo:before-fetch-request` for
    # the results frame (and hides it again on render) — so a slow semantic query
    # reads as "working". aria-hidden: the sr-only "Searching…" text in the busy
    # spinner already announces the state.
    def progress_bar
      div(
        class: "absolute inset-x-0 bottom-0 h-0.5 overflow-hidden hidden",
        data: { email_search_target: "progress" },
        aria_hidden: "true"
      ) do
        div(class: "h-full w-1/3 rounded-full bg-accent-600 animate-search-progress")
      end
    end

    def input_row
      div(class: "flex items-center gap-1.5 px-2.5 py-1.5 relative") do
        # Search icon — hidden when busy; spinner shown instead.
        span(class: "flex-shrink-0", data: { email_search_target: "searchIcon" }) do
          icon(SEARCH_ICON, "w-3.5 h-3.5 text-gray-400")
        end
        # Busy spinner — hidden by default, shown during frame fetch.
        span(
          class: "flex-shrink-0 hidden",
          aria_live: "polite",
          data: { email_search_target: "spinner" }
        ) do
          # Match the search icon footprint (arbitrary values so they reliably
          # beat the size preset's w-4 h-4 in Tailwind's output order).
          render(Campbooks::Spinner.new(size: :sm, class: "w-[0.875rem] h-[0.875rem]"))
          span(class: "sr-only") { t(".searching") }
        end
        div(class: "flex-1 min-w-0") do
          input(
            type: "text",
            name: "q",
            value: @search_params[:q],
            placeholder: t(".placeholder"),
            autocomplete: "off",
            autocapitalize: "off",
            spellcheck: "false",
            role: "combobox",
            aria_expanded: "false",
            aria_controls: "email-search-suggestions",
            aria_autocomplete: "list",
            class: "block w-full bg-transparent border-0 p-0 text-xs text-gray-700 dark:text-gray-200 placeholder:text-gray-400 focus:ring-0 focus:outline-none",
            data: {
              email_search_target: "query",
              action: "input->email-search#scheduleSubmit keydown->email-search#handleKeydown focus->email-search#openSuggestions blur->email-search#closeSuggestionsSoon"
            }
          )
        end
        clear_link if @search_params[:q].present?
        filters_button
      end
    end

    # Modifier typeahead panel — rows rendered by the email-search controller.
    def suggestions_panel
      div(
        id: "email-search-suggestions",
        role: "listbox",
        class: "absolute left-0 right-0 top-full z-30 mt-1 mx-1.5 hidden rounded-xl border border-gray-100 bg-card shadow-lg overflow-hidden",
        data: { email_search_target: "suggestions" }
      ) do
        div(
          class: "max-h-64 overflow-y-auto overscroll-contain py-1",
          data: { email_search_target: "suggestionsList" }
        )
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

    # Modifier catalog passed as a JSON Stimulus value. Each entry describes one
    # modifier token and enough metadata for the JS to render typeahead rows.
    def catalog
      [
        { token: "from:",     type: "remote", url: helpers.search_contacts_path, description: t(".suggest.from") },
        { token: "to:",       type: "remote", url: helpers.search_contacts_path, description: t(".suggest.to") },
        { token: "subject:",  type: "text",   description: t(".suggest.subject") },
        { token: "has:",      type: "enum",   description: t(".suggest.has"),
          values: [ { value: "attachment", label: t(".suggest.values.attachment") } ] },
        { token: "is:",       type: "enum",   description: t(".suggest.is"),
          values: %w[unread read pinned].map { |v| { value: v, label: t(".suggest.values.#{v}") } } },
        { token: "after:",    type: "date",   description: t(".suggest.after"),  hint: "YYYY-MM-DD" },
        { token: "before:",   type: "date",   description: t(".suggest.before"), hint: "YYYY-MM-DD" },
        { token: "tag:",      type: "enum",   description: t(".suggest.tag"),
          values: @tags.map { |tag| { value: tag.name, label: tag.name } } },
        { token: "folder:",   type: "enum",   description: t(".suggest.folder"),
          values: @folders.filter_map { |f|
            name = (f[:name] || f["name"]).presence
            { value: name, label: name } if name
          } },
        { token: "category:", type: "enum",   description: t(".suggest.category"),
          values: Campbooks::CategoryChip::CATEGORIES.map { |c| { value: c.to_s, label: t("components.category_chip.labels.#{c}") } } },
        { token: "priority:", type: "enum",   description: t(".suggest.priority"),
          values: %w[low medium high].map { |v| { value: v, label: t(".suggest.values.#{v}") } } },
        { token: "account:",  type: "enum",   description: t(".suggest.account"),
          values: @accounts.map { |a| { value: account_value(a), label: account_value(a) } } }
      ]
    end

    def account_value(account)
      account.respond_to?(:email_address) ? account.email_address.to_s : account.to_s
    end

    def normalize(params)
      return {} if params.nil?
      (params.respond_to?(:to_unsafe_h) ? params.to_unsafe_h : params).symbolize_keys
    end
  end
end
