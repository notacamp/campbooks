# frozen_string_literal: true

module Api
  module V1
    # Public API for email templates. Templates are workspace-scoped; all reads
    # are gated by `templates:read` and writes/apply by `templates:write`. The
    # whole resource 404s when Features.email_templates? is false (production-
    # readiness gate), and writes additionally require the :email_templates
    # entitlement (billing gate).
    class EmailTemplatesController < BaseController
      before_action :require_feature
      before_action -> { doorkeeper_authorize! :"templates:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"templates:write" }, only: [ :create, :update, :destroy, :apply ]
      before_action -> { require_entitlement!(:email_templates) },   only: [ :create, :update, :destroy, :apply ]
      before_action :set_template, only: [ :show, :update, :destroy, :apply ]

      def index
        scope = Current.workspace.email_templates.recent
        @pagy, records = pagy(scope, limit: per_page)
        render_page(records.map { |t| EmailTemplateSerializer.new(t).as_json }, @pagy)
      end

      def show
        render_data(EmailTemplateSerializer.new(@template, detail: true).as_json)
      end

      def create
        template = Current.workspace.email_templates.new(template_params)
        template.save!
        assign_document_templates(template)
        render_data(EmailTemplateSerializer.new(template, detail: true).as_json, status: :created)
      end

      def update
        @template.update!(template_params)
        assign_document_templates(@template)
        render_data(EmailTemplateSerializer.new(@template, detail: true).as_json)
      end

      def destroy
        @template.destroy
        head :no_content
      end

      # Render the template with the supplied variables: returns subject, body,
      # and any PDF attachment descriptors. Does NOT send an email.
      def apply
        variables = submitted_variables
        result = EmailTemplates::Applier.call(template: @template, variables: variables, user: Current.user)
        render_data({
          email_template_id: @template.id,
          subject: result.subject,
          body_html: result.body_html,
          variables: variables,
          attachments: result.attachments
        })
      end

      private

      def require_feature
        head :not_found unless Features.email_templates?
      end

      def set_template
        @template = Current.workspace.email_templates.find(params[:id])
      end

      # Only accept the variable keys this template actually declares or uses, so
      # the API can't smuggle arbitrary keys into the Liquid context.
      def submitted_variables
        raw = params[:variables]
        return {} unless raw.respond_to?(:permit)

        allowed = (
          @template.variable_definitions.filter_map { |v| v["key"] } +
          @template.extract_used_variables
        ).uniq
        raw.permit(*allowed).to_h
      end

      # Attach chosen document templates, scoped to THIS workspace so a forged id
      # cannot link another workspace's document template.
      def assign_document_templates(template)
        return unless params.key?(:document_template_ids)

        ids = Array(params[:document_template_ids]).reject(&:blank?)
        template.document_template_ids = Current.workspace.document_templates.where(id: ids).ids
      end

      def template_params
        params.permit(:name, :description, :subject, :body_html)
      end
    end
  end
end
