# frozen_string_literal: true

module Api
  module V1
    # Public API for document templates. All actions are gated behind the
    # Features.document_templates? flag; writes also require the
    # :document_templates entitlement (not available on the free plan).
    class DocumentTemplatesController < BaseController
      before_action :require_feature
      before_action -> { doorkeeper_authorize! :"templates:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"templates:write" }, only: [ :create, :update, :destroy, :render_pdf, :send_email ]
      before_action -> { require_entitlement!(:document_templates) }, only: [ :create, :update, :destroy, :render_pdf, :send_email ]
      before_action :set_template, only: [ :show, :update, :destroy, :render_pdf, :send_email ]

      def index
        scope = Current.workspace.document_templates.recent
        @pagy, records = pagy(scope, limit: per_page)
        render_page(records.map { |t| DocumentTemplateSerializer.new(t).as_json }, @pagy)
      end

      def show
        render_data(DocumentTemplateSerializer.new(@template, detail: true).as_json)
      end

      def create
        template = Current.workspace.document_templates.new(template_params)
        template.save!
        render_data(DocumentTemplateSerializer.new(template, detail: true).as_json, status: :created)
      end

      def update
        @template.update!(template_params)
        render_data(DocumentTemplateSerializer.new(@template, detail: true).as_json)
      end

      def destroy
        @template.destroy
        head :no_content
      end

      # Fills the template with the supplied variables and streams the resulting
      # PDF. No email is sent — use send_email for that.
      def render_pdf
        result = DocumentTemplates::Sender.call(template: @template, variables: variable_params, to_address: nil)

        if result.ok
          send_data result.pdf,
                    filename: "#{@template.name.parameterize}.pdf",
                    type: "application/pdf",
                    disposition: "attachment"
        else
          render_api_error("render_failed",
                           result.error || "Could not render the template.",
                           status: :unprocessable_entity)
        end
      end

      # Fills the template, renders a PDF, and emails it to the given address.
      def send_email
        return unless ensure_sendable_account(params[:email_account_id])

        result = DocumentTemplates::Sender.call(
          template: @template,
          variables: variable_params,
          to_address: params.require(:to_address),
          subject: params[:subject],
          body: params[:body],
          user: Current.user,
          email_account_id: params[:email_account_id]
        )

        if result.ok
          render_data({ ok: true, email_message_id: result.email_message&.id }, status: :created)
        else
          render_api_error("send_failed",
                           result.error || "Could not send.",
                           status: :unprocessable_entity)
        end
      end

      private

      def require_feature
        head(:not_found) unless Features.document_templates?
      end

      def set_template
        @template = Current.workspace.document_templates.find(params[:id])
      end

      # True when the acting user may send from `account_id`; otherwise renders
      # 403 and returns false so callers can `return unless ensure_sendable_account`.
      def ensure_sendable_account(account_id)
        return true if account_id.present? && Current.user.sendable_email_accounts.exists?(id: account_id)

        render_api_error("no_sendable_account",
                         "You can't send from that email account.", status: :forbidden)
        false
      end

      def template_params
        params.permit(:name, :description, :html_content)
      end

      def variable_params
        params.permit(variables: {})[:variables] || {}
      end
    end
  end
end
