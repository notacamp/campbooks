# frozen_string_literal: true

module InboxSettings
  # Inbox rules management panel. Rules are workspace-scoped deterministic
  # filters that tag, archive, mark read, or file incoming email automatically.
  # Mirrors the structure of InboxSettings::TagsController: renders into the
  # inbox_settings_panel Turbo Frame; CRUD responds with Turbo Streams.
  class RulesController < BaseController
    include ActionView::RecordIdentifier # bare dom_id(...) in turbo_stream helpers

    before_action :set_rule, only: [ :show, :edit, :update, :destroy, :toggle, :run ]

    def index
      @rules = Current.workspace.email_rules
        .includes(:tags, :mail_folder, :runs)
        .order(created_at: :desc)
    end

    def show
      # Renders the rule row in its own turbo_frame — used by the run-progress
      # poller to refresh a single row without reloading the whole list.
    end

    def new
      @rule = Current.workspace.email_rules.new(prefill_criteria)
    end

    def create
      @rule = Current.workspace.email_rules.new(rule_params)
      @rule.created_by = Current.user
      @rule.tag_ids = permitted_tag_ids

      if @rule.save
        enqueue_run_on_existing if run_on_existing?
        # -> create.turbo_stream.erb
      else
        render_form_errors
      end
    end

    def edit
    end

    def update
      # tag_ids= persists immediately on a saved record, so wrap the whole
      # update in a transaction: a validation failure must not half-apply.
      ActiveRecord::Base.transaction do
        @rule.tag_ids = permitted_tag_ids
        @rule.update!(rule_params)
      end

      enqueue_run_on_existing if run_on_existing?
      # -> update.turbo_stream.erb
    rescue ActiveRecord::RecordInvalid
      render_form_errors
    end

    def destroy
      @rule.destroy
      # -> destroy.turbo_stream.erb
    end

    def toggle
      @rule.update!(enabled: !@rule.enabled?)
      # -> toggle.turbo_stream.erb
    end

    def run
      @run = @rule.runs.create!(
        workspace: Current.workspace,
        started_by: Current.user,
        status: :queued
      )
      EmailRuleRunJob.perform_later(@run.id)
      # -> run.turbo_stream.erb
    end

    # GET /inbox_settings/rules/match_count.json?criteria[from]=…
    # Returns { count: N } — never persists, workspace-scoped.
    def match_count
      rule = Current.workspace.email_rules.new(criteria: criteria_from_params)

      # No meaningful conditions means the rule would match all mail — return 0
      # without hitting the DB so the UI can show a safe placeholder.
      conditions = rule.criteria.except("email_account_id")
        .reject { |_, v| v.nil? || (v.is_a?(Array) && v.empty?) || v == false }

      if conditions.empty?
        render json: { count: 0 }
        return
      end

      render json: { count: EmailRules::Matcher.new(rule).count }
    rescue => e
      Rails.logger.error("[InboxSettings::RulesController#match_count] #{e.message}")
      render json: { count: 0 }
    end

    # POST /inbox_settings/rules/:rule_id/runs/:id/undo
    def undo
      @rule = Current.workspace.email_rules.find(params[:rule_id])
      @run  = @rule.runs.find(params[:id])

      EmailRules::UndoRun.call(@run)

      render turbo_stream: [
        turbo_stream.replace(dom_id(@rule)) {
          render_to_string(
            partial: "inbox_settings/rules/rule",
            locals: { rule: @rule.reload }
          )
        },
        notify_stream(t(".undone"))
      ]
    rescue ArgumentError => e
      render turbo_stream: notify_stream(e.message, severity: :error)
    rescue ActiveRecord::RecordNotFound
      render turbo_stream: notify_stream(t(".not_found"), severity: :error),
             status: :not_found
    end

    private

    def set_rule
      @rule = Current.workspace.email_rules.find(params[:id])
    end

    # Strong params for the rule itself (name, booleans, folder). Criteria are
    # built separately in rule_params to normalize the flat form fields into the
    # jsonb criteria hash before assignment.
    def rule_params
      scalar = params.require(:email_rule).permit(:name, :archive, :mark_read, :mail_folder_id, :enabled)
      scalar.merge(criteria: criteria_from_rule_params)
    end

    # Build the criteria hash from the nested email_rule[criteria][...] fields.
    def criteria_from_rule_params
      cp = params.dig(:email_rule, :criteria) || {}
      build_criteria(cp)
    end

    # Build the criteria hash from the top-level criteria[...] fields (match_count).
    def criteria_from_params
      cp = params[:criteria] || {}
      build_criteria(cp)
    end

    def build_criteria(cp)
      criteria = {}
      %w[from to subject body].each do |field|
        val = cp[field].to_s.strip
        criteria[field] = val if val.present?
      end
      cats = Array(cp[:category]).reject(&:blank?)
      criteria["category"] = cats if cats.any?
      acct = cp[:email_account_id].to_s.strip
      criteria["email_account_id"] = acct if acct.present?
      criteria["has_attachment"] = true if cp[:has_attachment] == "1"
      criteria
    end

    def permitted_tag_ids
      Array(params.dig(:email_rule, :tag_ids)).reject(&:blank?)
    end

    def run_on_existing?
      params.dig(:email_rule, :run_on_existing) == "1"
    end

    def enqueue_run_on_existing
      run = @rule.runs.create!(
        workspace: Current.workspace,
        started_by: Current.user,
        status: :queued
      )
      EmailRuleRunJob.perform_later(run.id)
    end

    # Criteria prefilled from ?prefill[from]=… query params (empty-state starters).
    def prefill_criteria
      return {} unless params[:prefill].present?

      criteria = {}
      %w[from subject].each do |field|
        val = params.dig(:prefill, field).to_s.strip
        criteria[field] = val if val.present?
      end
      cat = params.dig(:prefill, :category).to_s.strip
      criteria["category"] = [ cat ] if cat.present?

      { criteria: criteria }
    end

    def render_form_errors
      render turbo_stream: turbo_stream.update(
        "inbox_settings_rule_form",
        partial: "inbox_settings/rules/form",
        locals: { rule: @rule }
      ), status: :unprocessable_entity
    end
  end
end
