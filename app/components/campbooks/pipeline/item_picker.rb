# frozen_string_literal: true

module Campbooks
  module Pipeline
    # The "add items" modal, loaded into the pipeline_picker Turbo Frame. Lists
    # documents/emails not yet in the pipeline (filtered by the search box, which
    # debounce-submits back into the frame); clicking "Add" posts a membership and
    # the response refreshes the board and drops the row from this list.
    class ItemPicker < Campbooks::Base
      def initialize(pipeline:, items:, query:)
        @pipeline = pipeline
        @items = items
        @query = query
      end

      def view_template
        div(
          class: "fixed inset-0 z-50 flex items-start justify-center p-4 sm:items-center bg-black/40",
          tabindex: "-1",
          data: { controller: "pipeline-picker", action: "click->pipeline-picker#backdropClose keydown->pipeline-picker#keydown" }
        ) do
          div(class: "relative flex w-full max-w-lg max-h-[80vh] flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl") do
            header
            search_form
            list
          end
        end
      end

      private

      def header
        div(class: "flex items-center justify-between gap-3 border-b border-border px-4 py-3 flex-shrink-0") do
          h2(class: "text-sm font-semibold text-foreground truncate") { t(".title", pipeline: @pipeline.name) }
          render(Campbooks::IconButton.new(aria_label: t("shared.actions.close"), size: :sm, data: { action: "click->pipeline-picker#close" })) do
            render(Campbooks::Icon.new("no-symbol", css_class: "w-4 h-4"))
          end
        end
      end

      def search_form
        form(action: helpers.new_pipeline_membership_path(@pipeline), method: "get", class: "border-b border-border p-3 flex-shrink-0", data: { turbo_frame: "pipeline_picker" }) do
          input(
            type: "search",
            name: "q",
            value: @query,
            placeholder: t(".search_placeholder"),
            autocomplete: "off",
            class: "w-full rounded-lg border border-border bg-card px-3 py-2 text-sm text-foreground shadow-sm placeholder:text-muted-foreground focus:border-accent-500 focus:ring-1 focus:ring-accent-500",
            data: { pipeline_picker_target: "search", action: "input->pipeline-picker#search" }
          )
        end
      end

      def list
        div(class: "flex-1 overflow-y-auto p-2") do
          if @items.empty?
            p(class: "px-2 py-10 text-center text-[13px] text-muted-foreground") { t(".empty") }
          else
            @items.each { |item| item_row(item) }
          end
        end
      end

      def item_row(item)
        div(id: helpers.dom_id(item, :picker), class: "flex items-center justify-between gap-3 rounded-lg px-2 py-2 hover:bg-muted") do
          div(class: "min-w-0") do
            span(class: "block truncate text-[13px] font-medium text-foreground") { item_title(item) }
            span(class: "block truncate text-[11px] text-muted-foreground") { item_subtitle(item) }
          end
          add_form(item)
        end
      end

      def add_form(item)
        form(action: helpers.pipeline_memberships_path(@pipeline), method: "post", class: "flex-shrink-0") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "item_type", value: item.class.name)
          input(type: "hidden", name: "item_id", value: item.id)
          render(Campbooks::Button.new(variant: :outline, size: :xs, type: :submit)) { t(".add") }
        end
      end

      def document?(item) = item.is_a?(Document)

      def item_title(item)
        document?(item) ? item.display_title : (item.subject.presence || t(".no_subject"))
      end

      def item_subtitle(item)
        if document?(item)
          item.classification&.name.to_s
        else
          item.from_address.to_s
        end
      end
    end
  end
end
