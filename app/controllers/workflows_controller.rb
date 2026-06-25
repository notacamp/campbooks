class WorkflowsController < ApplicationController
  before_action :require_workflows_enabled
  before_action :require_authentication
  before_action :set_workflow, only: [ :edit, :update, :destroy, :toggle, :add_step, :remove_step, :regenerate_webhook ]

  def index
    @workflows = Current.workspace.workflows.order(created_at: :desc)
  end

  def new
    redirect_to workflows_path
  end

  def edit
    set_edit_assigns
  end

  # Renders one Liquid input per writable property of a Notion database, for the
  # notion_create_database_item step. Loaded into the builder via the notion-fields
  # Stimulus controller (fetch → innerHTML). Reads the database schema live.
  def notion_fields
    prefix = params[:prefix].to_s
    values = parse_notion_values(params[:values])
    schema = nil
    error = nil

    integration = Current.workspace.notion_integrations.active.find_by(id: params[:integration_id])
    if integration && params[:database_id].present?
      schema = Notion::Client.new(integration).get_database(params[:database_id])
      if schema.is_a?(Hash) && (schema["object"] == "error" || schema["message"].present?)
        error = schema["message"] || schema["error"]
        schema = nil
      end
    end

    render partial: "notion_fields", locals: { schema: schema, prefix: prefix, values: values, error: error }
  rescue => e
    render partial: "notion_fields", locals: { schema: nil, prefix: params[:prefix].to_s, values: {}, error: e.message }
  end

  def create
    return if require_entitlement!(:workflows)

    workflow = Current.workspace.workflows.create!(
      name: "New Workflow",
      trigger_type: "email_received",
      created_by: Current.user
    )
    redirect_to edit_workflow_path(workflow), success: "Workflow created."
  end

  def update
    if unsendable_account_selected?
      set_edit_assigns
      flash.now[:error] = "You can only send from an email account you have send access to."
      return render :edit, status: :unprocessable_entity
    end

    if @workflow.update(workflow_params)
      redirect_to edit_workflow_path(@workflow), success: "Workflow saved."
    else
      set_edit_assigns
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @workflow.destroy
    redirect_to workflows_path, success: "Workflow deleted."
  end

  def toggle
    @workflow.update!(enabled: !@workflow.enabled)
    redirect_to workflows_path, success: @workflow.enabled ? "Workflow enabled." : "Workflow disabled."
  end

  def regenerate_webhook
    @workflow.regenerate_webhook_token!
    redirect_to edit_workflow_path(@workflow), success: "New webhook URL generated. Update the URL anywhere it's already in use."
  end

  def add_step
    step_type = params[:step_type] == "condition" ? "condition" : "action"
    position = (@workflow.steps.maximum(:position) || -1) + 1
    @workflow.steps.create!(
      position: position,
      step_type: step_type,
      action_type: step_type == "action" ? requested_action_type : nil
    )
    redirect_to edit_workflow_path(@workflow), success: "Step added."
  end

  def remove_step
    step = @workflow.steps.find(params[:step_id])
    step.destroy
    redirect_to edit_workflow_path(@workflow), success: "Step removed."
  end

  helper_method :trigger_summary, :step_label, :step_summary

  private

  def set_edit_assigns
    @steps = @workflow.steps.ordered
    # The "Send from" picker only offers accounts the editing user may send from;
    # a workflow's send authority is the configurer's, checked here at build time.
    @email_accounts = Current.user.sendable_email_accounts.active
    @connections = Current.workspace.connections.ordered
    @document_types = Current.workspace.document_types.order(:name)
    @notion_integrations = Current.workspace.notion_integrations.active.order(:created_at)
    @recent_executions = @workflow.executions.limit(5)
    @webhook_url = @workflow.webhook_token.present? ? webhook_url(@workflow.webhook_token) : nil
    @liquid_variables = liquid_variables_for(@workflow)
  end

  def liquid_variables_for(workflow)
    if workflow.webhook?
      {
        "payload.event" => "A field from the JSON body",
        "payload.id" => "Another payload field",
        "headers.User-Agent" => "An inbound request header",
        "query.token" => "A query-string parameter"
      }
    elsif workflow.event_trigger?
      {
        "event.name" => "The event type (e.g. document.approved)",
        "event.payload.field" => "A field from the event payload",
        "event.subject.type" => "The linked record's type",
        "event.subject.id" => "The linked record's id",
        "event.actor.label" => "Who caused the event"
      }
    else
      {
        "email.from" => "Sender email address",
        "email.to" => "Recipient addresses",
        "email.subject" => "Email subject",
        "email.body" => "Email body",
        "email.received_at" => "Received timestamp",
        "email.account_email" => "Your email account",
        "documents[0].filename" => "First document filename",
        "documents[0].document_type" => "First document type"
      }
    end
  end

  def trigger_summary(workflow)
    if workflow.webhook?
      "When an external service calls the webhook URL"
    elsif workflow.event_trigger?
      name = workflow.trigger_config.with_indifferent_access[:event_name].to_s
      return "When a selected event happens" if name.blank?

      label = Events::Registry.definition(name)&.label || name
      "When the \"#{label}\" event happens"
    else
      config = workflow.trigger_config.with_indifferent_access
      case config[:has_documents]
      when "yes" then "Only emails with documents"
      when "no" then "Only emails without documents"
      else "All incoming emails"
      end
    end
  end

  def step_label(step)
    case step.step_type
    when "condition"
      config = step.config.with_indifferent_access
      field = config[:field].to_s.humanize.presence || "Field"
      operator = config[:operator].to_s.humanize.downcase.presence || "equals"
      value = config[:value].to_s
      "If #{field} #{operator} '#{value}'"
    when "action"
      step.heading
    else
      step.step_type.humanize
    end
  end

  def step_summary(step)
    config = step.config.with_indifferent_access
    case step.action_type
    when "send_email"
      account = Current.workspace.email_accounts.find_by(id: config[:email_account_id]) if config[:email_account_id].present?
      "From: #{account&.email_address || 'No account selected'}"
    when "http_request"
      url = config[:url].to_s
      "#{(config[:http_method].presence || 'POST').upcase} #{url.presence || 'no URL set'}"
    when "slack_message"
      config[:webhook_url].present? ? "Posts to Slack" : "No Slack URL set"
    when "discord_message"
      config[:webhook_url].present? ? "Posts to Discord" : "No Discord URL set"
    when "custom_action"
      connection = Current.workspace.connections.find_by(id: config[:connection_id]) if config[:connection_id].present?
      connection ? "#{(config[:http_method].presence || 'POST').upcase} #{connection.name}#{config[:path]}" : "No integration selected"
    when "email_action"
      tool = config[:email_tool].to_s
      tool.present? ? "#{tool.humanize} (triggering email)" : "No action selected"
    when "emit_event"
      name = config[:event_name].to_s
      name.present? ? "Emits \"#{name}\"" : "No event name set"
    end
  end

  def set_workflow
    @workflow = Current.workspace.workflows.find(params[:id])
  end

  # Honour an explicit action_type from the "add" menu, falling back to send_email.
  def requested_action_type
    type = params[:action_type].to_s
    WorkflowStep::ACTION_TYPES.include?(type) ? type : "send_email"
  end

  # Reject a save where any step picks an email account the editor can't send
  # from — defends the freeze-at-build authority even against a crafted POST that
  # bypasses the (already narrowed) dropdown.
  def unsendable_account_selected?
    steps = workflow_params[:steps_attributes]
    return false if steps.blank?

    sendable_ids = Current.user.sendable_email_accounts.ids
    steps.values.any? do |attrs|
      account_id = attrs.dig(:config, :email_account_id)
      account_id.present? && sendable_ids.exclude?(account_id.to_i)
    end
  end

  def workflow_params
    params.require(:workflow).permit(
      :name, :description, :enabled, :trigger_type,
      trigger_config: [ :has_documents, :event_name ],
      steps_attributes: [
        :id, :position, :step_type, :action_type, :_destroy,
        config: permitted_config_keys
      ]
    )
  end

  # Condition keys plus the union of every action's config keys, sourced from
  # Workflows::ActionRegistry — so a newly registered action's fields are
  # permitted automatically, with no second place to update.
  def parse_notion_values(raw)
    return {} if raw.blank?
    parsed = JSON.parse(raw)
    parsed.is_a?(Hash) ? parsed : {}
  rescue JSON::ParserError
    {}
  end

  def permitted_config_keys
    scalars = (%w[field operator value] + Workflows::ActionRegistry.config_keys).uniq.map(&:to_sym)
    # notion_create_database_item stores a per-property hash (Prop name => Liquid
    # template) built from the live DB schema, so permit it as a nested open hash.
    scalars + [ { notion_properties: {} } ]
  end
end
