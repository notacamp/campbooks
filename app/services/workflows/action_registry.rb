module Workflows
  # Single source of truth for workflow ACTION step types.
  #
  # Each Definition carries everything the rest of the system needs to know about
  # an action: how the builder UI presents it (the StepPicker card + the action
  # <select>), which config keys are persisted and permitted, and how the
  # Executor runs it — either by building an outbound HTTP request (`build:`) or
  # via a custom runner (`run:`). Both name an Executor instance method.
  #
  # Adding an action type means adding ONE entry here. WorkflowStep's constants,
  # WorkflowsController's strong params, the StepPicker catalog, and the
  # Executor's dispatch all derive from this registry, so they can never drift.
  module ActionRegistry
    # A configurable field on an action. `type` drives the form widget and, for
    # :liquid types, signals that the Executor renders it through Liquid. `key`
    # is what's stored in WorkflowStep#config and permitted in the controller.
    #
    #   type:    :liquid | :select | :headers | :account_select | :string | :text
    #   options: choices for :select
    def self.field(key, type, **opts)
      { key: key.to_s, type: type, **opts }
    end

    Definition = Struct.new(
      :key, :label, :group, :icon, :description, :config_schema, :build, :run,
      keyword_init: true
    ) do
      # Outbound HTTP actions name a request `build`er; everything else a `run`ner.
      def http?
        !build.nil?
      end

      def config_keys
        config_schema.map { |fld| fld[:key] }
      end

      # Shape consumed by Campbooks::StepPicker.
      def picker_card
        {
          group: group, key: key, title: label, icon: icon,
          step_type: "action", action_type: key, description: description
        }
      end

      # [value, label] pair for the action <select> in Campbooks::WorkflowStepForm.
      def select_option
        [ key, label ]
      end
    end

    DEFINITIONS = [
      Definition.new(
        key: "send_email", label: "Send Email", group: :action, icon: :mail,
        description: "Send an email from a connected account",
        run: :execute_send_email,
        config_schema: [
          field(:email_account_id, :account_select, label: "Send from"),
          field(:to_template,      :liquid, label: "To",      placeholder: "e.g. {{ email.from }}"),
          field(:subject_template, :liquid, label: "Subject", placeholder: "e.g. Re: {{ email.subject }}"),
          field(:body_template,    :liquid, label: "Body",    placeholder: "Use Liquid to build the email body"),
          field(:cc_template,      :liquid, label: "CC",      placeholder: "Optional"),
          field(:bcc_template,     :liquid, label: "BCC",     placeholder: "Optional")
        ]
      ),
      Definition.new(
        key: "http_request", label: "HTTP Request", group: :action, icon: :bolt,
        description: "Call any URL with a custom method, headers and body",
        build: :build_generic_request,
        config_schema: [
          field(:http_method,  :select, label: "Method", options: %w[POST GET PUT PATCH DELETE]),
          field(:url,          :liquid, label: "URL", placeholder: "https://api.example.com/hook"),
          field(:headers,      :headers, label: "Headers", hint: "One per line as Key: Value. Values support Liquid."),
          field(:content_type, :string, label: "Content-Type"),
          field(:body,         :liquid, label: "Body", placeholder: 'JSON or text. e.g. {"event":"{{ payload.event }}"}')
        ]
      ),
      Definition.new(
        key: "slack_message", label: "Slack Message", group: :action, icon: :chat,
        description: "Post a message to Slack via an incoming webhook",
        build: :build_slack_request,
        config_schema: [
          field(:webhook_url, :liquid, label: "Slack webhook URL", placeholder: "https://hooks.slack.com/services/..."),
          field(:text,        :liquid, label: "Message", placeholder: "New document received: {{ documents[0].filename }}")
        ]
      ),
      Definition.new(
        key: "discord_message", label: "Discord Message", group: :action, icon: :hash,
        description: "Post a message to Discord via an incoming webhook",
        build: :build_discord_request,
        config_schema: [
          field(:webhook_url, :liquid, label: "Discord webhook URL", placeholder: "https://discord.com/api/webhooks/..."),
          field(:content,     :liquid, label: "Message", placeholder: "New document received: {{ documents[0].filename }}"),
          field(:username,    :text,   label: "Override username (optional)", placeholder: "Campbooks")
        ]
      ),
      Definition.new(
        key: "custom_action", label: "Custom Action", group: :action, icon: :link,
        description: "Call a saved integration with a path, headers and body",
        build: :build_custom_action_request,
        config_schema: [
          field(:connection_id, :connection_select, label: "Integration"),
          field(:http_method,   :select, label: "Method", options: %w[POST GET PUT PATCH DELETE]),
          field(:path,          :liquid, label: "Path", placeholder: "/v1/refunds"),
          field(:headers,       :headers, label: "Headers", hint: "Optional. The integration's auth header is added automatically."),
          field(:body,          :liquid, label: "Body", placeholder: '{ "id": "{{ payload.id }}" }')
        ]
      ),
      Definition.new(
        key: "email_action", label: "Email Action", group: :action, icon: :inbox,
        description: "Tag, archive, or forward the email that triggered the workflow",
        run: :execute_email_action,
        config_schema: [
          field(:email_tool, :email_tool_select, label: "Action"),
          field(:tag_name,   :liquid, label: "Tag name", placeholder: "e.g. invoice"),
          field(:to_address, :liquid, label: "Forward to", placeholder: "e.g. {{ email.from }}")
        ]
      ),
      Definition.new(
        key: "emit_event", label: "Emit Event", group: :action, icon: :bolt,
        description: "Record a custom event — can trigger other workflows",
        run: :execute_emit_event,
        config_schema: [
          field(:event_name,    :liquid, label: "Event name", placeholder: "e.g. invoice.flagged"),
          field(:event_payload, :liquid, label: "Payload (JSON)", placeholder: '{ "amount": "{{ event.payload.amount }}" }')
        ]
      ),
      Definition.new(
        key: "google_drive_create_folder", label: "Create Drive Folder", group: :action, icon: :folder,
        description: "Create a folder in Google Drive",
        run: :execute_drive_create_folder,
        config_schema: [
          field(:folder_name,       :liquid, label: "Folder name", placeholder: "e.g. {{ email.subject }}"),
          field(:parent_folder_id,  :liquid, label: "Parent folder ID", placeholder: "Optional — blank for My Drive root")
        ]
      ),
      Definition.new(
        key: "google_drive_upload", label: "Upload to Drive", group: :action, icon: :upload,
        description: "Upload the triggering email's attachments to Google Drive",
        run: :execute_drive_upload,
        config_schema: [
          field(:parent_folder_id, :liquid, label: "Destination folder ID", placeholder: "Optional — blank for My Drive root")
        ]
      ),
      Definition.new(
        key: "notion_create_page", label: "Create Notion Page", group: :action, icon: :document,
        description: "Create a subpage under a Notion page",
        run: :execute_notion_create_page,
        config_schema: [
          field(:notion_integration_id, :notion_integration_select, label: "Notion workspace"),
          field(:parent_page_id, :liquid, label: "Parent page ID", placeholder: "The page to nest under"),
          field(:page_title,     :liquid, label: "Title", placeholder: "e.g. {{ email.subject }}"),
          field(:page_content,   :liquid, label: "Body", placeholder: "Optional page content"),
          field(:attach_attachments, :select, label: "Attach email files", options: %w[no yes])
        ]
      ),
      Definition.new(
        key: "notion_create_database_item", label: "Create Notion Item", group: :action, icon: :document,
        description: "Add a row to a Notion database and upload attachments",
        run: :execute_notion_create_database_item,
        config_schema: [
          field(:notion_integration_id, :notion_integration_select, label: "Notion workspace"),
          field(:notion_database_id, :liquid, label: "Database ID", placeholder: "The target database"),
          # notion_properties is a per-field hash rendered from the live DB schema in
          # the builder; it's permitted as a nested hash in WorkflowsController.
          field(:notion_file_property, :liquid, label: "Files property", placeholder: "Optional — property name for attachments")
        ]
      )
    ].freeze

    INDEX = DEFINITIONS.index_by(&:key).freeze

    class << self
      def all
        DEFINITIONS
      end

      def definition(key)
        INDEX[key.to_s]
      end

      def keys
        DEFINITIONS.map(&:key)
      end

      # { action_type => label } — used by WorkflowStep.action_labels as the fallback.
      def labels
        DEFINITIONS.to_h { |d| [ d.key, d.label ] }
      end

      # Action types that make an outbound HTTP call (share the executor's
      # SSRF guard + response-recording plumbing).
      def http_keys
        DEFINITIONS.select(&:http?).map(&:key)
      end

      def select_options
        DEFINITIONS.map(&:select_option)
      end

      def picker_cards
        DEFINITIONS.map(&:picker_card)
      end

      # Union of every action's config keys — the action half of the strong
      # params permit-list.
      def config_keys
        DEFINITIONS.flat_map(&:config_keys).uniq
      end
    end
  end
end
