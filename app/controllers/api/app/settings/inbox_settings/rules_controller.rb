# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Inbox rule CRUD + toggle/run/undo/match_count.
        class RulesController < Api::App::BaseController
          before_action :set_rule, only: %i[show update destroy toggle run]

          # GET /api/app/inbox_settings/rules
          def index
            rules = current_workspace.email_rules
                                     .includes(:tags, :mail_folder, :runs)
                                     .order(created_at: :desc)
            render_data rules.map { |r| serialize(r) }
          end

          # GET /api/app/inbox_settings/rules/:id
          def show
            render_data serialize(@rule)
          end

          # POST /api/app/inbox_settings/rules
          def create
            rule = current_workspace.email_rules.new(rule_params)
            rule.created_by = current_user
            rule.tag_ids = permitted_tag_ids

            if rule.save
              enqueue_run_on_existing(rule) if run_on_existing?
              render_data serialize(rule), status: :created
            else
              render_error("invalid", rule.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # PATCH /api/app/inbox_settings/rules/:id
          def update
            ActiveRecord::Base.transaction do
              @rule.tag_ids = permitted_tag_ids
              @rule.update!(rule_params)
            end
            enqueue_run_on_existing(@rule) if run_on_existing?
            render_data serialize(@rule)
          rescue ActiveRecord::RecordInvalid => e
            render_error("invalid", e.record.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end

          # DELETE /api/app/inbox_settings/rules/:id
          def destroy
            @rule.destroy
            render json: {}, status: :no_content
          end

          # PATCH /api/app/inbox_settings/rules/:id/toggle
          def toggle
            @rule.update!(enabled: !@rule.enabled?)
            render_data serialize(@rule)
          end

          # POST /api/app/inbox_settings/rules/:id/run
          def run
            run_record = @rule.runs.create!(
              workspace: current_workspace,
              started_by: current_user,
              status: :queued
            )
            EmailRuleRunJob.perform_later(run_record.id)
            render json: { data: { run_id: run_record.id, status: "queued" } }, status: :accepted
          end

          # GET /api/app/inbox_settings/rules/match_count
          def match_count
            rule = current_workspace.email_rules.new(criteria: criteria_from_params)
            conditions = rule.criteria.except("email_account_id")
                            .reject { |_, v| v.nil? || (v.is_a?(Array) && v.empty?) || v == false }

            if conditions.empty?
              return render json: { data: { count: 0 } }
            end

            render json: { data: { count: EmailRules::Matcher.new(rule).count } }
          rescue => e
            Rails.logger.error("[Api::App::Settings::InboxSettings::RulesController#match_count] #{e.message}")
            render json: { data: { count: 0 } }
          end

          # POST /api/app/inbox_settings/rules/:rule_id/runs/:id/undo
          def undo
            rule = current_workspace.email_rules.find(params[:rule_id])
            run  = rule.runs.find(params[:id])
            EmailRules::UndoRun.call(run)
            render json: { data: { undone: true } }
          rescue ArgumentError => e
            render_error("invalid", e.message, status: :unprocessable_entity)
          rescue ActiveRecord::RecordNotFound
            render_not_found
          end

          private

          def set_rule
            @rule = current_workspace.email_rules.find(params[:id])
          end

          def rule_params
            scalar = params.require(:email_rule).permit(:name, :archive, :mark_read, :mail_folder_id, :enabled)
            scalar.merge(criteria: criteria_from_rule_params)
          end

          def criteria_from_rule_params
            cp = params.dig(:email_rule, :criteria) || {}
            build_criteria(cp)
          end

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
            criteria["has_attachment"] = true if cp[:has_attachment].in?([ "1", true, "true" ])
            criteria
          end

          def permitted_tag_ids
            Array(params.dig(:email_rule, :tag_ids)).reject(&:blank?)
          end

          def run_on_existing?
            params.dig(:email_rule, :run_on_existing).in?([ "1", true, "true" ])
          end

          def enqueue_run_on_existing(rule)
            run_record = rule.runs.create!(
              workspace: current_workspace,
              started_by: current_user,
              status: :queued
            )
            EmailRuleRunJob.perform_later(run_record.id)
          end

          def serialize(rule)
            Api::App::Settings::RuleSerializer.new(rule).as_json
          end
        end
      end
    end
  end
end
