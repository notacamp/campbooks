# frozen_string_literal: true

module Campbooks
  # The structured-filter controls for the inbox search bar. Rendered *inside* the
  # EmailSearchBar's <form>, so every control submits with it; the bar's Stimulus
  # controller auto-submits on change. Not a standalone form.
  class EmailFilterPanel < Campbooks::Base
    def initialize(folders: [], accounts: [], tags: [], active: {}, **attrs)
      @folders = folders
      @accounts = accounts
      @tags = tags
      @active = active.respond_to?(:to_unsafe_h) ? active.to_unsafe_h.symbolize_keys : (active || {}).symbolize_keys
      @attrs = attrs
    end

    def view_template
      div(class: class_names("space-y-3", @attrs.delete(:class)), **@attrs) do
        scalar_fields
        toggles
        accounts_section if @accounts.size > 1
        tags_section if @tags.any?
      end
    end

    private

    def scalar_fields
      # Names are passed as Strings, not Symbols: Campbooks::Input/Select render a
      # Symbol value through Phlex, which dasherises underscores (:date_from →
      # name="date-from") and would no longer match the controller's permitted
      # params. Strings render verbatim.
      div(class: "grid grid-cols-1 sm:grid-cols-2 gap-x-3 gap-y-2") do
        render(Campbooks::Select.new("folder", label: t(".folder"), options: folder_options,
          selected: @active[:folder], include_blank: t(".all_folders")))
        render(Campbooks::Select.new("category", label: t(".category"), options: category_options,
          selected: @active[:category], include_blank: t(".any_category")))
        render(Campbooks::Input.new("sender", label: t(".sender"), value: @active[:sender],
          placeholder: t(".sender_placeholder"), rounded: :md))
        render(Campbooks::Input.new("domain", label: t(".domain"), value: @active[:domain],
          placeholder: t(".domain_placeholder"), rounded: :md))
        render(Campbooks::Input.new("date_from", type: :date, label: t(".date_from"), value: @active[:date_from], rounded: :md))
        render(Campbooks::Input.new("date_to", type: :date, label: t(".date_to"), value: @active[:date_to], rounded: :md))
        render(Campbooks::Select.new("priority", label: t(".priority"), options: priority_options,
          selected: @active[:priority], include_blank: t(".any_priority")))
      end
    end

    def toggles
      div(class: "flex flex-wrap items-center gap-4") do
        render(Campbooks::Toggle.new(name: "has_attachment", label: t(".has_attachment"),
          checked: @active[:has_attachment].to_s == "1", value: "1"))
        render(Campbooks::Toggle.new(name: "unread", label: t(".unread"),
          checked: @active[:unread].to_s == "1", value: "1"))
      end
    end

    def accounts_section
      selected = Array(@active[:account_ids]).map(&:to_s)
      section(t(".accounts")) do
        div(class: "flex flex-col gap-1.5") do
          @accounts.each do |account|
            render(Campbooks::Checkbox.new("account_ids[]", label: account.email_address,
              value: account.id, checked: selected.include?(account.id.to_s)))
          end
        end
      end
    end

    def tags_section
      selected = Array(@active[:tag_ids]).map(&:to_s)
      section(t(".tags")) do
        tag_match_radios
        input(
          type: "text",
          placeholder: t(".tag_filter_placeholder"),
          class: "block w-full rounded-md border-gray-300 shadow-sm text-xs focus:border-accent-500 focus:ring-accent-500 mb-1.5",
          data: { action: "input->email-search#filterTags" }
        )
        div(class: "flex flex-col gap-1 max-h-40 overflow-y-auto") do
          @tags.each do |tag|
            label(
              class: "flex items-center gap-2 cursor-pointer",
              data: { email_search_target: "tagOption", tag_name: tag.name.to_s.downcase }
            ) do
              input(type: "checkbox", name: "tag_ids[]", value: tag.id, checked: selected.include?(tag.id.to_s),
                class: "w-3.5 h-3.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500")
              render(Campbooks::ColorDot.new(color: tag.color, size: :sm))
              span(class: "text-sm text-gray-700 truncate") { tag.name }
            end
          end
        end
      end
    end

    def tag_match_radios
      current = @active[:tag_match].to_s == "all" ? "all" : "any"
      div(class: "flex items-center gap-3 mb-1.5") do
        %w[any all].each do |mode|
          label(class: "flex items-center gap-1.5 cursor-pointer") do
            input(type: "radio", name: "tag_match", value: mode, checked: current == mode,
              class: "w-3.5 h-3.5 border-gray-300 text-accent-600 focus:ring-accent-500")
            span(class: "text-xs text-gray-600") { t(".tag_match_#{mode}") }
          end
        end
      end
    end

    def section(title, &block)
      div(class: "space-y-1.5") do
        span(class: "block text-[10px] font-semibold uppercase tracking-wide text-gray-400") { title }
        yield
      end
    end

    def folder_options
      @folders.map { |f| [ f[:name], f[:name] ] }
    end

    def category_options
      Campbooks::CategoryChip::CATEGORIES.map { |c| [ t("components.category_chip.labels.#{c}"), c.to_s ] }
    end

    def priority_options
      EmailMessage.ai_priorities.keys.map { |k| [ t(".priority_#{k}"), k ] }
    end
  end
end
