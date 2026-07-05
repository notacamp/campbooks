# frozen_string_literal: true

module Campbooks
  module Digests
    # The single-page create/edit form for a scheduled digest. Covers name,
    # per-source sections, schedule picker (Stimulus digest-schedule), delivery
    # toggles, and AI settings. The hidden digest[first_run_at] is written by
    # the Stimulus controller on change and submit with a full ISO-8601 offset.
    class Form < Campbooks::Base
      WINDOW_OPTIONS = [ 7, 14, 30 ].freeze

      def initialize(digest:, preset: nil)
        @digest = digest
        @preset = preset
      end

      def view_template
        form(
          action: form_action,
          method: :post,
          data: { controller: "digest-schedule" }
        ) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          unless @digest.new_record?
            input(type: "hidden", name: "_method", value: "patch")
          end
          # Marks a full-form submit: the controller only rebuilds config["sources"]
          # when present, so partial submits (the row's enabled toggle) can't wipe it.
          input(type: "hidden", name: "digest[sources_submitted]", value: "1")
          if @preset
            input(type: "hidden", name: "digest[preset_key]", value: @preset.key)
          end
          input(
            type: "hidden",
            name: "digest[first_run_at]",
            data: { "digest-schedule-target" => "firstRunAt" },
            value: @digest.next_run_at&.iso8601
          )

          div(class: "space-y-6 max-w-2xl") do
            name_section
            sources_section
            schedule_section
            delivery_section
            ai_section
            form_actions
          end
        end
      end

      private

      attr_reader :digest

      def form_action
        @digest.new_record? ? helpers.digests_path : helpers.digest_path(@digest)
      end

      def section_card(title_text, &block)
        div(class: "rounded-2xl border border-border bg-card p-5") do
          h3(class: "mb-4 text-sm font-semibold text-foreground") { title_text }
          yield
        end
      end

      def name_section
        section_card(t(".name_section")) do
          render Campbooks::Input.new(
            "digest[name]",
            label: t(".name_label"),
            value: @digest.name,
            placeholder: t(".name_placeholder"),
            required: true
          )
        end
      end

      def sources_section
        section_card(t(".sources_section")) do
          # ::-prefixed — bare Digests:: resolves to Campbooks::Digests:: in here.
          available = begin
            ::Digests::Sources.available_keys(@digest.workspace || Current.workspace)
          rescue StandardError
            ::Digests::Sources::KEYS
          end
          available.each { |key| source_row(key) }
        end
      end

      def source_row(source_key)
        cfg     = @digest.sources.find { |s| s["type"] == source_key } || {}
        enabled = @digest.sources.any? { |s| s["type"] == source_key }
        field_id = "source_#{source_key}"

        div(class: "mb-4 border-b border-border pb-4 last:mb-0 last:border-0 last:pb-0") do
          div(class: "flex items-center gap-3") do
            input(
              type: "checkbox",
              name: "digest[source_#{source_key}]",
              id: field_id,
              value: "1",
              checked: enabled,
              class: "h-4 w-4 rounded border-border text-foreground focus:ring-foreground"
            )
            label(for: field_id, class: "cursor-pointer text-sm font-medium text-foreground") do
              helpers.t("digests.sections.#{source_key}")
            end
          end

          div(
            id: "source_params_#{source_key}",
            class: "ml-7 mt-3 space-y-3",
            hidden: !enabled
          ) do
            source_params(source_key, cfg)
          end
        end
      end

      def source_params(source_key, cfg)
        case source_key
        when "emails"
          render Campbooks::Input.new(
            "digest[emails_query]",
            label: t(".emails_query_label"),
            value: cfg["query"].to_s,
            placeholder: t(".emails_query_placeholder"),
            hint: t(".emails_query_hint")
          )
        when "calendar"
          render Campbooks::Select.new(
            "digest[calendar_window_days]",
            label: t(".window_days_label"),
            options: window_options,
            selected: (cfg["window_days"] || 7).to_s
          )
        when "tasks"
          div(class: "space-y-3") do
            render Campbooks::Select.new(
              "digest[tasks_window_days]",
              label: t(".window_days_label"),
              options: window_options,
              selected: (cfg["window_days"] || 7).to_s
            )
            toggle_row("digest[tasks_include_overdue]", t(".include_overdue"), cfg["include_overdue"] == true)
          end
        when "reminders"
          render Campbooks::Select.new(
            "digest[reminders_window_days]",
            label: t(".window_days_label"),
            options: window_options,
            selected: (cfg["window_days"] || 7).to_s
          )
        when "documents"
          document_types_field(cfg)
        end
      end

      def window_options
        WINDOW_OPTIONS.map { |d| [ t(".window_days_option", count: d), d.to_s ] }
      end

      def document_types_field(cfg)
        selected_types = Array(cfg["document_types"]).map(&:downcase)
        workspace = @digest.workspace || Current.workspace
        all_types = workspace&.document_types&.order(:name)&.pluck(:name)&.map(&:downcase) || []

        div(class: "space-y-2") do
          label(class: "block text-sm font-medium text-gray-700") { t(".document_types_label") }
          if all_types.any?
            div(class: "flex flex-wrap gap-2") do
              all_types.each do |type_name|
                cb_id = "dt_#{type_name.gsub(/\W/, '_')}"
                div(class: "flex items-center gap-1.5") do
                  input(
                    type: "checkbox",
                    name: "digest[document_types][]",
                    id: cb_id,
                    value: type_name,
                    checked: selected_types.include?(type_name),
                    class: "h-4 w-4 rounded border-border text-foreground focus:ring-foreground"
                  )
                  label(for: cb_id, class: "text-sm text-foreground") { type_name.humanize }
                end
              end
            end
          else
            p(class: "text-sm text-muted-foreground") { t(".no_document_types") }
          end
        end
      end

      def schedule_section
        section_card(t(".schedule_section")) do
          hint      = @preset&.schedule_hint
          freq_val  = @digest.rrule.presence || "FREQ=WEEKLY"
          wday_val  = (hint&.fetch(:wday, nil) || 1).to_s
          # Kernel#format is shadowed by Phlex — use String#% instead.
          hour_val  = "%02d" % (hint&.fetch(:hour, 8) || 8)
          min_val   = "%02d" % (hint&.fetch(:min, 0) || 0)

          div(
            class: "space-y-4",
            data: {
              "digest-schedule-freq"         => freq_val,
              "digest-schedule-wday"         => wday_val,
              "digest-schedule-hour"         => hour_val,
              "digest-schedule-min"          => min_val,
              "digest-schedule-existing-iso" => @digest.next_run_at&.iso8601
            }
          ) do
            render Campbooks::Select.new(
              "digest[rrule]",
              label: t(".frequency_label"),
              options: [
                [ t(".freq_daily"),   "FREQ=DAILY" ],
                [ t(".freq_weekly"),  "FREQ=WEEKLY" ],
                [ t(".freq_monthly"), "FREQ=MONTHLY" ]
              ],
              selected: freq_val,
              data: { "digest-schedule-target" => "frequency", action: "change->digest-schedule#update" }
            )

            div(
              data: { "digest-schedule-target" => "weekdayRow" },
              hidden: !freq_val.include?("WEEKLY")
            ) do
              render Campbooks::Select.new(
                "digest[wday]",
                label: t(".weekday_label"),
                options: (0..6).map { |d| [ helpers.t("date.day_names")[d], d.to_s ] },
                selected: wday_val,
                data: { "digest-schedule-target" => "weekday", action: "change->digest-schedule#update" }
              )
            end

            div(class: "space-y-1") do
              label(class: "block text-sm font-medium text-foreground") { t(".time_label") }
              div(class: "flex items-center gap-2") do
                input(
                  type: "number", min: "0", max: "23", value: hour_val,
                  class: "w-20 rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground shadow-sm focus:border-accent-500 focus:ring-accent-500",
                  data: { "digest-schedule-target" => "hour", action: "change->digest-schedule#update" }
                )
                span(class: "text-muted-foreground") { ":" }
                input(
                  type: "number", min: "0", max: "59", step: "15", value: min_val,
                  class: "w-20 rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground shadow-sm focus:border-accent-500 focus:ring-accent-500",
                  data: { "digest-schedule-target" => "minute", action: "change->digest-schedule#update" }
                )
              end
            end

            p(
              class: "text-[13px] text-muted-foreground",
              data: { "digest-schedule-target" => "preview" }
            ) do
              if @digest.next_run_at
                t(".next_run_preview", time: helpers.l(@digest.next_run_at, format: :long))
              else
                t(".computing_schedule")
              end
            end
          end
        end
      end

      def delivery_section
        section_card(t(".delivery_section")) do
          div(class: "space-y-4") do
            toggle_row("digest[deliver_by_email]", t(".deliver_email"), @digest.deliver_by_email)
            toggle_row("digest[show_in_feed]", t(".show_in_feed"), @digest.show_in_feed)
          end
        end
      end

      def ai_section
        section_card(t(".ai_section")) do
          div(class: "space-y-4") do
            toggle_row("digest[ai_enabled]", t(".ai_enabled"), @digest.ai_enabled)
            render Campbooks::Textarea.new(
              "digest[ai_instructions]",
              label: t(".ai_instructions_label"),
              placeholder: t(".ai_instructions_placeholder"),
              rows: 3,
              hint: t(".ai_instructions_hint"),
              value: @digest.ai_instructions.to_s
            )
          end
        end
      end

      def form_actions
        div(class: "flex items-center justify-end gap-3 pt-2") do
          a(
            href: @digest.new_record? ? helpers.digests_path : helpers.digest_path(@digest),
            class: "rounded-xl border border-border bg-background px-4 py-2 text-sm font-semibold text-foreground shadow-sm transition-colors hover:bg-muted"
          ) { t("shared.actions.cancel") }
          button(
            type: "submit",
            class: "rounded-xl bg-foreground px-4 py-2 text-sm font-semibold text-background shadow-sm transition-colors hover:bg-foreground/80"
          ) do
            @digest.new_record? ? t(".create_button") : t(".update_button")
          end
        end
      end

      # Toggle with an explicit "1" value (the bare checkbox would submit "on")
      # and a hidden "0" companion so unchecked state reaches the controller as
      # an explicit "0" instead of an absent key.
      def toggle_row(name, label_text, checked)
        div(class: "flex items-center justify-between") do
          span(class: "text-sm text-foreground") { label_text }
          input(type: "hidden", name: name, value: "0")
          render Campbooks::Toggle.new(name: name, checked: !!checked, value: "1")
        end
      end
    end
  end
end
