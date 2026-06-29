# frozen_string_literal: true

module Campbooks
  # A single email tag, rendered as a colored pill. One chip style across the
  # inbox (thread rows, search results) and the email detail picker, so tags read
  # consistently everywhere — replacing the ad-hoc chip markup that used to be
  # duplicated across partials and components.
  #
  # Set `removable: true` and pass `remove_data:` (the Stimulus action + params)
  # for the detail picker's × button, e.g.:
  #
  #   Campbooks::TagChip.new(tag:, size: :md, removable: true, remove_data: {
  #     "action" => "email-tags#remove",
  #     "email-tags-tag-id-param" => tag.id,
  #     "email-tags-external-param" => tag.external?
  #   })
  class TagChip < Campbooks::Base
    SIZE = {
      sm: "text-[10px] px-1.5 py-0.5",
      md: "text-[11px] px-2 py-0.5"
    }.freeze

    def initialize(tag:, size: :md, removable: false, remove_data: {}, **attrs)
      @tag = tag
      @size = size
      @removable = removable
      @remove_data = remove_data
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      span(
        class: class_names(
          "inline-flex items-center gap-1 rounded font-medium max-w-[160px] select-none",
          SIZE.fetch(@size, SIZE[:md]), custom_class
        ),
        style: "background-color: #{@tag.color}20; color: #{@tag.color}",
        **@attrs
      ) do
        span(class: "truncate") { @tag.name }
        remove_button if @removable
      end
    end

    private

    def remove_button
      button(
        type: "button",
        aria_label: t(".remove", name: @tag.name),
        class: "inline-flex items-center justify-center min-w-[24px] min-h-[24px] -my-1 -mr-1.5 ml-0.5 rounded hover:bg-black/10 hover:opacity-70 leading-none text-[13px]",
        data: @remove_data
      ) { raw(safe("&times;")) }
    end
  end
end
