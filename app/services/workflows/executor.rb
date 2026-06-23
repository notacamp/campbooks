module Workflows
  # Runs a workflow's steps in order against a TriggerContext. Conditions that
  # evaluate false halt the run; actions perform their side effect and record
  # the outcome on the execution step. Any raised error fails the execution.
  class Executor
    def self.call(workflow, context)
      new(workflow, context).call
    end

    def initialize(workflow, context)
      @workflow = workflow
      @context = context
      @renderer = LiquidRenderer.new(context.liquid_context)
    end

    def call
      execution = create_execution
      execution.update!(started_at: Time.current)

      @workflow.steps.ordered.each do |step|
        step_execution = execution.execution_steps.create!(
          workflow_step: step,
          status: :running,
          started_at: Time.current,
          input_data: @context.step_input
        )

        case step.step_type
        when "condition"
          passed = ConditionEvaluator.evaluate(step, @context)
          step_execution.update!(
            status: :completed,
            output_data: { "result" => passed },
            completed_at: Time.current
          )
          break unless passed
        when "action"
          execute_action(step, step_execution)
        end
      end

      execution.update!(status: :completed, completed_at: Time.current)
      execution
    rescue StandardError => e
      execution&.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    private

    def create_execution
      WorkflowExecution.create!(
        workflow: @workflow,
        workspace: @workflow.workspace,
        status: :running,
        trigger_data: @context.trigger_data
      )
    end

    # Dispatch is driven by Workflows::ActionRegistry: an action either builds an
    # outbound HTTP request (`build`, routed through execute_http) or names a
    # custom runner (`run`). Adding an action type touches the registry and, only
    # if it needs new logic, its build/run method below.
    def execute_action(step, step_execution)
      config = step.config.with_indifferent_access
      definition = Workflows::ActionRegistry.definition(step.action_type)
      return unless definition

      if definition.http?
        execute_http(send(definition.build, config), step_execution)
      else
        send(definition.run, config, step_execution)
      end
    end

    # --- HTTP-backed actions -------------------------------------------------

    def execute_http(request, step_execution)
      result = HttpClient.call(**request)

      step_execution.update!(
        status: result[:ok] ? :completed : :failed,
        completed_at: Time.current,
        output_data: {
          "request" => { "method" => request[:method], "url" => request[:url] },
          "response" => { "status" => result[:status], "body" => result[:body].to_s.truncate(2000) }
        },
        error_message: result[:ok] ? nil : http_error(result)
      )

      raise "HTTP request to #{request[:url]} failed (#{http_error(result)})" unless result[:ok]
    end

    def http_error(result)
      result[:error].presence || "HTTP #{result[:status]}"
    end

    def build_generic_request(config)
      method = (config[:http_method].presence || "POST").to_s.upcase
      headers = parse_headers(config[:headers])
      headers["Content-Type"] ||= config[:content_type].presence || "application/json"

      body = bodyless?(method) ? nil : @renderer.render(config[:body].to_s)

      {
        method: method,
        url: @renderer.render(config[:url].to_s),
        headers: headers,
        body: body
      }
    end

    def build_slack_request(config)
      {
        method: "POST",
        url: @renderer.render(config[:webhook_url].to_s),
        headers: { "Content-Type" => "application/json" },
        body: { text: @renderer.render(config[:text].to_s) }.to_json
      }
    end

    def build_discord_request(config)
      payload = { content: @renderer.render(config[:content].to_s) }
      username = @renderer.render(config[:username].to_s)
      payload[:username] = username if username.present?

      {
        method: "POST",
        url: @renderer.render(config[:webhook_url].to_s),
        headers: { "Content-Type" => "application/json" },
        body: payload.to_json
      }
    end

    def build_custom_action_request(config)
      # Re-scope to the workflow's own workspace at run time so a stored/crafted
      # id can never reach a connection outside this workspace.
      connection = @workflow.workspace.connections.find_by(id: config[:connection_id])
      raise "Integration not found or not in this workspace" unless connection

      method = (config[:http_method].presence || "POST").to_s.upcase
      # The connection's auth header is merged last so a step can never override
      # the credential. The resolved URL still passes through UrlGuard via execute_http.
      headers = parse_headers(config[:headers]).merge(connection.auth_headers)
      headers["Content-Type"] ||= "application/json"

      path = @renderer.render(config[:path].to_s)
      url = connection.base_url
      url += path.start_with?("/") ? path : "/#{path}" if path.present?

      {
        method: method,
        url: url,
        headers: headers,
        body: bodyless?(method) ? nil : @renderer.render(config[:body].to_s)
      }
    end

    def bodyless?(method)
      %w[GET HEAD DELETE].include?(method)
    end

    # Headers are configured as newline-separated "Key: Value" lines; each value
    # is Liquid-rendered so secrets/IDs can come from the trigger payload.
    def parse_headers(raw)
      return {} if raw.blank?

      raw.to_s.lines.each_with_object({}) do |line, acc|
        key, value = line.split(":", 2)
        next if key.blank? || value.nil?

        acc[key.strip] = @renderer.render(value.strip)
      end
    end

    # --- email_action (bridge into EmailActions) -----------------------------

    # Runs an EmailActions tool (tag/archive/forward…) on the email that
    # triggered the workflow, acting as the workflow's owner so EmailActions'
    # per-user permission gates apply. Only email-triggered workflows carry an
    # email; everything else fails closed.
    def execute_email_action(config, step_execution)
      email_message = @context.email_message
      raise "Email actions only run on email-triggered workflows" unless email_message

      tool = config[:email_tool].to_s
      args = {
        tag_name: @renderer.render(config[:tag_name].to_s),
        to_address: @renderer.render(config[:to_address].to_s)
      }.reject { |_key, value| value.blank? }

      result = EmailActions.run(tool, email_message: email_message, args: args, user: @workflow.created_by)

      step_execution.update!(
        status: result[:success] ? :completed : :failed,
        completed_at: Time.current,
        output_data: { "tool" => tool, "message" => result[:message], "result" => result[:result] },
        error_message: result[:success] ? nil : result[:message]
      )

      raise "Email action '#{tool}' failed: #{result[:message]}" unless result[:success]
    end

    # --- emit_event (records a domain Event) ---------------------------------

    # Records a domain event from inside a workflow. The workflow is the actor,
    # the triggering subject (email/document/…) carries through, and source_event
    # links the causation chain so emit→trigger→emit loops are bounded (see
    # Event::MAX_CHAIN_DEPTH). The new event can itself trigger other
    # `event`-triggered workflows.
    def execute_emit_event(config, step_execution)
      name = @renderer.render(config[:event_name].to_s).strip
      raise "Event name is required" if name.blank?

      payload = parse_event_payload(config[:event_payload])

      event = Events.publish(
        name,
        workspace: @workflow.workspace,
        actor: @workflow,
        subject: @context.subject,
        caused_by: @context.source_event,
        payload: payload
      )

      step_execution.update!(
        status: :completed,
        completed_at: Time.current,
        output_data: { "event_id" => event&.id, "name" => name, "payload" => payload }
      )
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    def parse_event_payload(raw)
      rendered = @renderer.render(raw.to_s).strip
      return {} if rendered.blank?

      parsed = JSON.parse(rendered)
      parsed.is_a?(Hash) ? parsed : { "value" => parsed }
    rescue JSON::ParserError => e
      raise "Invalid JSON payload: #{e.message}"
    end

    # --- Google Drive --------------------------------------------------------

    def execute_drive_create_folder(config, step_execution)
      account = drive_account!
      name = @renderer.render(config[:folder_name].to_s).strip
      raise "Folder name is required" if name.blank?
      parent = @renderer.render(config[:parent_folder_id].to_s).strip.presence

      folder = Integrations::Drive::FolderCreator.new(account).call(name: name, parent_id: parent)

      step_execution.update!(
        status: :completed, completed_at: Time.current,
        output_data: { "folder_id" => folder.id, "name" => folder.name }
      )
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    def execute_drive_upload(config, step_execution)
      account = drive_account!
      files = trigger_files
      raise "No attachments on the triggering email to upload" if files.empty?
      parent = @renderer.render(config[:parent_folder_id].to_s).strip.presence

      results = Integrations::Drive::FileUploader.new(account).call(files: files, folder_id: parent)

      step_execution.update!(
        status: :completed, completed_at: Time.current,
        output_data: { "uploaded" => results.map { |r| { "id" => r.id, "name" => r.name } } }
      )
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    def drive_account!
      account = @workflow.workspace.google_drive_accounts.connected.first
      raise "Google Drive isn't connected for this workspace" unless account
      account
    end

    # --- Notion --------------------------------------------------------------

    def execute_notion_create_page(config, step_execution)
      integration = notion_integration!(config)
      parent_page_id = @renderer.render(config[:parent_page_id].to_s).strip
      raise "Parent page is required" if parent_page_id.blank?

      title = @renderer.render(config[:page_title].to_s).presence || "Untitled"
      content = @renderer.render(config[:page_content].to_s).presence
      files = config[:attach_attachments].to_s == "yes" ? trigger_files : []

      page = Integrations::Notion::PageCreator.new(integration).call(
        parent_page_id: parent_page_id, title: title, content: content, files: files
      )
      finish_notion(step_execution, page)
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    def execute_notion_create_database_item(config, step_execution)
      integration = notion_integration!(config)
      database_id = @renderer.render(config[:notion_database_id].to_s).strip
      raise "Notion database is required" if database_id.blank?

      inputs = notion_inputs_from_config(integration, database_id, config[:notion_properties])
      file_fields = notion_file_fields(config[:notion_file_property])

      page = Integrations::Notion::DatabaseItemCreator.new(integration).call(
        database_id: database_id, inputs: inputs, file_fields: file_fields
      )
      finish_notion(step_execution, page)
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    def notion_integration!(config)
      scope = @workflow.workspace.notion_integrations.active
      integration = config[:notion_integration_id].present? ? scope.find_by(id: config[:notion_integration_id]) : scope.first
      raise "Notion isn't connected for this workspace" unless integration
      integration
    end

    # Liquid-renders each per-field template, then maps the property to its live
    # schema type (never trust stored types). Unknown/blank properties are skipped.
    def notion_inputs_from_config(integration, database_id, props_config)
      return {} if props_config.blank?

      props = ::Notion::Client.new(integration).get_database(database_id)["properties"] || {}
      (props_config || {}).each_with_object({}) do |(name, template), acc|
        type = props.dig(name, "type")
        next unless type
        rendered = @renderer.render(template.to_s)
        acc[name] = { type: type, value: rendered } if rendered.present?
      end
    end

    def notion_file_fields(raw)
      prop = @renderer.render(raw.to_s).strip
      return {} if prop.blank?
      files = trigger_files
      files.empty? ? {} : { prop => files }
    end

    def finish_notion(step_execution, page)
      unless page && page["id"]
        raise((page && (page["message"] || page["error"])) || "Notion request failed")
      end
      step_execution.update!(
        status: :completed, completed_at: Time.current,
        output_data: { "page_id" => page["id"], "url" => page["url"] }
      )
    end

    # Attachments on the triggering email, as a FileSource (empty for non-email triggers).
    def trigger_files
      return [] unless @context.email_message
      Integrations::FileSource.for(email_message: @context.email_message)
    end

    # --- send_email ----------------------------------------------------------

    def execute_send_email(config, step_execution)
      # Re-scope to the workflow's own workspace at run time: defense-in-depth so a
      # stored/crafted id can never reach an account outside this workspace.
      account = @workflow.workspace.email_accounts.find_by(id: config[:email_account_id])
      raise "Email account not found or not sendable" unless account

      to_address = @renderer.render(config[:to_template].to_s)
      subject = @renderer.render(config[:subject_template].to_s)
      body = @renderer.render(config[:body_template].to_s)
      cc = @renderer.render(config[:cc_template].to_s)
      bcc = @renderer.render(config[:bcc_template].to_s)

      raise "Recipient (to) is required" if to_address.blank?

      result = account.mail_client.send_message(
        subject: subject.presence || "(no subject)",
        body: body,
        to_address: to_address,
        cc_address: cc.presence,
        attachments: email_attachments
      )

      record_sent_message(account, to_address, subject, body, cc, bcc, result)

      step_execution.update!(
        status: :completed,
        completed_at: Time.current,
        output_data: { sent: true, to: to_address, subject: subject }
      )
    rescue StandardError => e
      step_execution.update!(status: :failed, error_message: e.message, completed_at: Time.current)
      raise
    end

    # Only email triggers carry attachments; webhook-triggered emails send none.
    def email_attachments
      return [] unless @context.email_message

      @context.email_message.files.map do |file|
        blob = file.blob
        { filename: blob.filename.to_s, content: Base64.strict_encode64(blob.download) }
      end
    end

    def record_sent_message(account, to_address, subject, body, cc, bcc, result)
      sent_id = if result.is_a?(Hash)
        (result["data"] || result)&.dig("messageId") || result["id"]
      end

      thread = Emails::Threading.find_or_create_outbound(account, subject.presence || "(no subject)")

      thread.email_messages.create!(
        email_account: account,
        provider_message_id: sent_id || "workflow_#{SecureRandom.hex(8)}",
        provider_folder_id: "sent",
        from_address: account.email_address,
        to_address: to_address,
        cc_address: cc.presence,
        bcc_address: bcc.presence,
        subject: subject,
        body: body,
        received_at: Time.current,
        read: true,
        status: :processed
      )
    end
  end
end
