module Campbooks
  # Renders the expanded configuration form inside a workflow step card. Handles
  # every trigger and action variant; which fields show is driven by a type
  # <select> wired to the `workflow-type` Stimulus controller so switching types
  # updates the visible panel without a round-trip.
  class WorkflowStepForm < Campbooks::Base
    INPUT = "block w-full rounded-lg shadow-sm text-sm".freeze
    LABEL = "block text-sm font-medium text-foreground mb-1".freeze

    # Sourced from the registry so the action <select> always matches the picker
    # and the executor's known action types.
    ACTION_OPTIONS = Workflows::ActionRegistry.select_options.freeze

    # @param step_type [Symbol] :trigger, :condition, :action
    # @param config [Hash] current config values
    # @param type_value [String, nil] current trigger_type / action_type
    # @param type_field_name [String, nil] input name for the type <select>
    # @param field_prefix [String] prefix for config field names
    # @param trigger_type [String] the workflow's trigger (adapts conditions)
    # @param webhook_url [String, nil] full inbound URL for the webhook panel
    def initialize(step_type:, config: {}, type_value: nil, type_field_name: nil,
                   field_prefix: "", email_accounts: [], connections: [], document_types: [],
                   notion_integrations: [],
                   trigger_type: "email_received", webhook_url: nil, liquid_variables: {}, **attrs)
      @step_type = step_type
      @config = config.with_indifferent_access
      @type_value = type_value
      @type_field_name = type_field_name
      @field_prefix = field_prefix
      @email_accounts = email_accounts
      @connections = connections
      @document_types = document_types
      @notion_integrations = notion_integrations
      @trigger_type = trigger_type
      @webhook_url = webhook_url
      @liquid_variables = liquid_variables
      @attrs = attrs
    end

    def view_template
      div(class: "space-y-4", **@attrs) do
        case @step_type
        when :trigger then render_trigger_form
        when :condition then render_condition_form
        when :action then render_action_form
        end
      end
    end

    private

    # --- Trigger -------------------------------------------------------------

    def render_trigger_form
      div(data: { controller: "workflow-type" }) do
        type_select(trigger_options, current: @trigger_type, label: t(".trigger_label"))

        panel("email_received") { trigger_email_fields }
        panel("webhook") { trigger_webhook_fields }
        panel("event") { trigger_event_fields }
      end
    end

    # The catalog (Events::Registry) feeds a datalist for discoverability, while
    # the plain text input still allows a custom event name or a prefix wildcard
    # like "document.*".
    def trigger_event_fields
      div do
        label(for: f("event_name"), class: LABEL) { t(".event_name_label") }
        input(
          type: "text", name: f("event_name"), id: f("event_name"),
          value: @config[:event_name], class: "#{INPUT} font-mono",
          list: "workflow-event-options", placeholder: "document.approved", autocomplete: "off"
        )
        datalist(id: "workflow-event-options") do
          Events::Registry.all.each do |defn|
            option(value: defn.key) { defn.label }
          end
        end
        p(class: "mt-1 text-xs text-muted-foreground") { t(".event_name_hint") }
      end
    end

    def trigger_email_fields
      div do
        label(for: f("has_documents"), class: LABEL) { t(".filter_by_attachments") }
        select(name: f("has_documents"), id: f("has_documents"), class: INPUT) do
          option(value: "any", selected: @config[:has_documents].blank? || @config[:has_documents] == "any") { t(".has_documents.any") }
          option(value: "yes", selected: @config[:has_documents] == "yes") { t(".has_documents.yes") }
          option(value: "no", selected: @config[:has_documents] == "no") { t(".has_documents.no") }
        end
      end
    end

    def trigger_webhook_fields
      render(Campbooks::WebhookUrlField.new(
        url: @webhook_url,
        hint: t(".webhook_hint")
      ))
    end

    # --- Condition -----------------------------------------------------------

    def render_condition_form
      if @trigger_type == "email_received"
        render_document_condition
      else
        render_expression_condition
      end
    end

    def render_document_condition
      div(class: "grid grid-cols-1 gap-3 sm:grid-cols-3") do
        div do
          label(class: LABEL) { t(".field_label") }
          select(name: f("field"), class: INPUT) do
            option(value: "document_type", selected: true) { t(".document_type") }
          end
        end
        operator_select
        div do
          label(for: f("value"), class: LABEL) { t(".value_label") }
          if @document_types.any?
            select(name: f("value"), id: f("value"), class: INPUT) do
              option(value: "") { t(".select_placeholder") }
              @document_types.each do |dt|
                option(value: dt.name.downcase, selected: @config[:value]&.downcase == dt.name.downcase) { dt.name }
              end
            end
          else
            input(type: "text", name: f("value"), id: f("value"), value: @config[:value], class: INPUT, placeholder: "invoice")
          end
        end
      end
    end

    def render_expression_condition
      div(class: "space-y-3") do
        div do
          label(for: f("field"), class: LABEL) { t(".field_label") }
          input(type: "text", name: f("field"), id: f("field"), value: @config[:field], class: "#{INPUT} font-mono", placeholder: "payload.event")
          p(class: "mt-1 text-xs text-muted-foreground") { t(".field_hint") }
        end
        div(class: "grid grid-cols-1 gap-3 sm:grid-cols-2") do
          operator_select
          div do
            label(for: f("value"), class: LABEL) { t(".value_label") }
            input(type: "text", name: f("value"), id: f("value"), value: @config[:value], class: INPUT, placeholder: "invoice.paid")
          end
        end
      end
    end

    def operator_select
      div do
        label(for: f("operator"), class: LABEL) { t(".operator_label") }
        select(name: f("operator"), id: f("operator"), class: INPUT) do
          operator_options.each do |value, text|
            option(value: value, selected: @config[:operator] == value || (@config[:operator].blank? && value == "equals")) { text }
          end
        end
      end
    end

    # --- Action --------------------------------------------------------------

    def render_action_form
      div(data: { controller: "workflow-type" }) do
        type_select(ACTION_OPTIONS, current: @type_value, label: t(".action_label"))

        panel("send_email") { send_email_fields }
        panel("http_request") { http_request_fields }
        panel("slack_message") { slack_fields }
        panel("discord_message") { discord_fields }
        panel("custom_action") { custom_action_fields }
        panel("email_action") { email_action_fields }
        panel("emit_event") { emit_event_fields }
        panel("google_drive_create_folder") { drive_create_folder_fields }
        panel("google_drive_upload") { drive_upload_fields }
        panel("notion_create_page") { notion_create_page_fields }
        panel("notion_create_database_item") { notion_create_database_item_fields }
      end
    end

    def drive_create_folder_fields
      div(class: "space-y-4") do
        liquid_field("folder_name", t(".folder_name"), "e.g. {{ email.subject }}")
        liquid_field("parent_folder_id", t(".parent_folder_id"), t(".parent_folder_hint"))
      end
    end

    def drive_upload_fields
      div(class: "space-y-4") do
        liquid_field("parent_folder_id", t(".parent_folder_id"), t(".parent_folder_hint"))
        p(class: "text-xs text-muted-foreground") { t(".drive_upload_hint") }
      end
    end

    def notion_create_page_fields
      div(class: "space-y-4") do
        notion_integration_select
        liquid_field("parent_page_id", t(".parent_page_id"), t(".parent_page_hint"))
        liquid_field("page_title", t(".page_title"), "e.g. {{ email.subject }}")
        liquid_field("page_content", t(".page_content"), t(".optional"))
        div do
          label(for: f("attach_attachments"), class: LABEL) { t(".attach_attachments") }
          select(name: f("attach_attachments"), id: f("attach_attachments"), class: INPUT) do
            option(value: "no", selected: @config[:attach_attachments].to_s != "yes") { t(".attach_no") }
            option(value: "yes", selected: @config[:attach_attachments].to_s == "yes") { t(".attach_yes") }
          end
        end
      end
    end

    def notion_create_database_item_fields
      div(class: "space-y-4",
          data: {
            controller: "notion-fields",
            notion_fields_url_value: helpers.notion_fields_workflows_path,
            notion_fields_prefix_value: "#{@field_prefix}[notion_properties]",
            notion_fields_current_value: (@config[:notion_properties] || {}).to_json,
            notion_fields_loading_value: t(".load_loading"),
            notion_fields_error_value: t(".load_error")
          }) do
        notion_integration_select(target: "integration")
        div do
          label(for: f("notion_database_id"), class: LABEL) { t(".notion_database_id") }
          input(type: "text", name: f("notion_database_id"), id: f("notion_database_id"),
                value: @config[:notion_database_id], class: "#{INPUT} font-mono",
                placeholder: "1a2b3c4d…", data: { notion_fields_target: "database" })
          p(class: "mt-1 text-xs text-muted-foreground") { t(".notion_database_hint") }
        end
        button(type: "button",
               class: "inline-flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md border border-input bg-background hover:bg-accent hover:text-accent-foreground cursor-pointer",
               data: { action: "notion-fields#load" }) { t(".load_fields") }
        div(class: "rounded-lg border border-input p-3", data: { notion_fields_target: "fields" }) do
          p(class: "text-sm text-muted-foreground") { t(".load_prompt") }
        end
        liquid_field("notion_file_property", t(".notion_file_property"), t(".notion_file_property_hint"))
      end
    end

    def notion_integration_select(target: nil)
      div do
        label(for: f("notion_integration_id"), class: LABEL) { t(".notion_workspace") }
        if @notion_integrations.any?
          select(name: f("notion_integration_id"), id: f("notion_integration_id"), class: INPUT,
                 data: target ? { notion_fields_target: target } : {}) do
            @notion_integrations.each do |integ|
              option(value: integ.id, selected: @config[:notion_integration_id].to_s == integ.id.to_s) { integ.display_name }
            end
          end
        else
          input(type: "hidden", name: f("notion_integration_id"), value: @config[:notion_integration_id])
          p(class: "text-xs text-muted-foreground") do
            plain t(".no_notion_prefix")
            plain " "
            a(href: helpers.settings_integrations_notion_path, class: "text-foreground hover:text-muted-foreground") { t(".no_notion_link") }
          end
        end
      end
    end

    def emit_event_fields
      div(class: "space-y-4") do
        liquid_field("event_name", t(".event_name_label"), "e.g. invoice.flagged")
        liquid_field("event_payload", t(".event_payload_label"), '{ "amount": "{{ event.payload.amount }}" }')
        p(class: "text-xs text-muted-foreground") { t(".emit_event_hint") }
      end
    end

    def send_email_fields
      div(class: "space-y-4") do
        div do
          label(for: f("email_account_id"), class: LABEL) { t(".send_from") }
          select(name: f("email_account_id"), id: f("email_account_id"), class: INPUT) do
            option(value: "") { t(".select_email_account") }
            @email_accounts.each do |account|
              option(value: account.id, selected: @config[:email_account_id].to_s == account.id.to_s) { account.select_label }
            end
          end
        end
        liquid_field("to_template", t(".to"), "e.g. {{ email.from }}")
        liquid_field("subject_template", t(".subject"), "e.g. Re: {{ email.subject }}")
        liquid_field("body_template", t(".body"), t(".body_hint"))
        liquid_field("cc_template", t(".cc"), t(".optional"))
        liquid_field("bcc_template", t(".bcc"), t(".optional"))
      end
    end

    def http_request_fields
      div(class: "space-y-4") do
        div(class: "grid grid-cols-1 gap-3 sm:grid-cols-[140px_1fr]") do
          div do
            label(for: f("http_method"), class: LABEL) { t(".method") }
            select(name: f("http_method"), id: f("http_method"), class: INPUT) do
              %w[POST GET PUT PATCH DELETE].each do |m|
                option(value: m, selected: (@config[:http_method].presence || "POST") == m) { m }
              end
            end
          end
          liquid_field("url", t(".url"), "https://api.example.com/hook")
        end
        div do
          label(for: f("headers"), class: LABEL) { t(".headers") }
          textarea(name: f("headers"), id: f("headers"), rows: 2, class: "#{INPUT} font-mono", placeholder: "Authorization: Bearer xxx\nX-Custom: value") { @config[:headers].to_s }
          p(class: "mt-1 text-xs text-muted-foreground") { t(".headers_hint") }
        end
        liquid_field("body", t(".body"), 'JSON or text. e.g. {"event":"{{ payload.event }}"}')
      end
    end

    def slack_fields
      div(class: "space-y-4") do
        liquid_field("webhook_url", t(".slack_webhook_url"), "https://hooks.slack.com/services/...")
        liquid_field("text", t(".message"), "New document received: {{ documents[0].filename }}")
        slack_hint("https://api.slack.com/messaging/webhooks", t(".slack_hint"))
      end
    end

    def discord_fields
      div(class: "space-y-4") do
        liquid_field("webhook_url", t(".discord_webhook_url"), "https://discord.com/api/webhooks/...")
        liquid_field("content", t(".message"), "New document received: {{ documents[0].filename }}")
        div do
          label(for: f("username"), class: LABEL) { t(".override_username") }
          input(type: "text", name: f("username"), id: f("username"), value: @config[:username], class: INPUT, placeholder: "Campbooks")
        end
        slack_hint("https://support.discord.com/hc/en-us/articles/228383668", t(".discord_hint"))
      end
    end

    def custom_action_fields
      div(class: "space-y-4") do
        div do
          label(for: f("connection_id"), class: LABEL) { t(".integration") }
          if @connections.any?
            select(name: f("connection_id"), id: f("connection_id"), class: INPUT) do
              option(value: "") { t(".select_integration") }
              @connections.each do |conn|
                option(value: conn.id, selected: @config[:connection_id].to_s == conn.id.to_s) { conn.select_label }
              end
            end
          else
            input(type: "hidden", name: f("connection_id"), value: @config[:connection_id])
            p(class: "text-xs text-muted-foreground") do
              plain t(".no_integrations_prefix")
              a(href: helpers.settings_integrations_connections_path, class: "text-foreground hover:text-muted-foreground") { t(".no_integrations_link") }
              plain t(".no_integrations_suffix")
            end
          end
        end
        div(class: "grid grid-cols-1 gap-3 sm:grid-cols-[140px_1fr]") do
          div do
            label(for: f("http_method"), class: LABEL) { t(".method") }
            select(name: f("http_method"), id: f("http_method"), class: INPUT) do
              %w[POST GET PUT PATCH DELETE].each do |m|
                option(value: m, selected: (@config[:http_method].presence || "POST") == m) { m }
              end
            end
          end
          liquid_field("path", t(".path"), "/v1/refunds")
        end
        div do
          label(for: f("headers"), class: LABEL) { t(".headers") }
          textarea(name: f("headers"), id: f("headers"), rows: 2, class: "#{INPUT} font-mono", placeholder: "X-Custom: value") { @config[:headers].to_s }
          p(class: "mt-1 text-xs text-muted-foreground") { t(".custom_headers_hint") }
        end
        liquid_field("body", t(".body"), '{ "id": "{{ payload.id }}" }')
      end
    end

    def email_action_fields
      div(class: "space-y-4") do
        div do
          label(for: f("email_tool"), class: LABEL) { t(".email_tool") }
          select(name: f("email_tool"), id: f("email_tool"), class: INPUT) do
            option(value: "") { t(".select_email_tool") }
            EmailActions.tools_for(:workflow).each do |defn|
              option(value: defn.id, selected: @config[:email_tool] == defn.id) { defn.id.humanize }
            end
          end
          p(class: "mt-1 text-xs text-muted-foreground") { t(".email_action_hint") }
        end
        liquid_field("tag_name", t(".tag_name"), t(".tag_name_hint"))
        liquid_field("to_address", t(".forward_to"), t(".forward_to_hint"))
      end
    end

    # --- Shared helpers ------------------------------------------------------

    def type_select(options, current:, label:)
      return if @type_field_name.blank?

      div do
        label(for: type_field_id, class: LABEL) { label }
        select(
          name: @type_field_name,
          id: type_field_id,
          class: INPUT,
          data: { workflow_type_target: "select", action: "change->workflow-type#update" }
        ) do
          options.each do |value, text|
            option(value: value, selected: current.to_s == value) { text }
          end
        end
      end
    end

    def panel(type, &block)
      div(class: "mt-4", data: { workflow_type_target: "panel", workflow_type: type }, &block)
    end

    def liquid_field(field, label_text, hint)
      render(Campbooks::LiquidField.new(
        name: f(field), label: label_text, value: @config[field], hint: hint, variables: @liquid_variables
      ))
    end

    def slack_hint(url, text)
      p(class: "text-xs text-muted-foreground") do
        plain text
        plain " "
        a(href: url, target: "_blank", rel: "noopener", class: "text-foreground hover:text-muted-foreground") { t("shared.actions.learn_more") }
      end
    end

    def trigger_options
      [
        [ "email_received", t(".trigger_options.email_received") ],
        [ "webhook", t(".trigger_options.webhook") ],
        [ "event", t(".trigger_options.event") ]
      ]
    end

    def operator_options
      [
        [ "equals", t(".operator_options.equals") ],
        [ "not_equals", t(".operator_options.not_equals") ],
        [ "contains", t(".operator_options.contains") ],
        [ "not_contains", t(".operator_options.not_contains") ],
        [ "exists", t(".operator_options.exists") ],
        [ "not_exists", t(".operator_options.not_exists") ]
      ]
    end

    def f(field)
      "#{@field_prefix}[#{field}]"
    end

    def type_field_id
      @type_field_id ||= @type_field_name.to_s.gsub(/[\[\]]+/, "_").gsub(/_+$/, "")
    end
  end
end
