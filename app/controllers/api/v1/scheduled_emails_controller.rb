# frozen_string_literal: true

module Api
  module V1
    # Public API for scheduled (and recurring) email sends. Reads are workspace-
    # scoped via ScheduledEmail.accessible_to; writes additionally require the
    # :email_scheduling entitlement and that the acting user may send from the
    # chosen account (sendable_email_accounts), mirroring the web composer.
    class ScheduledEmailsController < BaseController
      before_action -> { doorkeeper_authorize! :"scheduled_emails:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"scheduled_emails:write" }, only: [ :create, :update, :destroy ]
      before_action -> { require_entitlement!(:email_scheduling) },         only: [ :create, :update, :destroy ]
      before_action :set_scheduled_email, only: [ :show, :update, :destroy ]
      before_action :require_editable!, only: [ :update, :destroy ]

      def index
        scope = ScheduledEmail.accessible_to(Current.user)
                              .includes(:email_account)
                              .order(Arel.sql("COALESCE(next_occurrence_at, scheduled_at) ASC"))
        scope = scope.where(status: params[:status]) if valid_status?(params[:status])
        @pagy, records = pagy(scope, limit: per_page)
        render_page(records.map { |r| ScheduledEmailSerializer.new(r).as_json }, @pagy)
      end

      def show
        render_data(ScheduledEmailSerializer.new(@scheduled_email, detail: true).as_json)
      end

      def create
        account = sendable_account(params[:email_account_id])
        return unless account

        scheduled_email = ScheduledEmail.new(scheduled_email_params)
        scheduled_email.email_account = account
        scheduled_email.workspace  = Current.workspace
        scheduled_email.created_by = Current.user
        scheduled_email.save!
        recalculate_next_occurrence(scheduled_email)

        render_data(ScheduledEmailSerializer.new(scheduled_email, detail: true).as_json, status: :created)
      end

      def update
        if params.key?(:email_account_id)
          account = sendable_account(params[:email_account_id])
          return unless account

          @scheduled_email.email_account = account
        end

        @scheduled_email.update!(scheduled_email_params)
        recalculate_next_occurrence(@scheduled_email)
        render_data(ScheduledEmailSerializer.new(@scheduled_email, detail: true).as_json)
      end

      # Cancel is a soft state change (status: cancelled), matching the web destroy.
      def destroy
        @scheduled_email.update!(status: :cancelled)
        render_data(ScheduledEmailSerializer.new(@scheduled_email, detail: true).as_json)
      end

      private

      def set_scheduled_email
        @scheduled_email = ScheduledEmail.accessible_to(Current.user).find(params[:id])
      end

      # Reads admit mailbox readers; mutating needs the creator or send access
      # on the account (403 — the record is visible, the action is denied).
      def require_editable!
        return if @scheduled_email.editable_by?(Current.user)

        render_api_error("scheduled_email_not_editable",
                         "You may not modify this scheduled email.", status: :forbidden)
      end

      # Mirrors the web ScheduledEmailsController: advance next_occurrence_at past
      # `now` for recurring rules, or pin it to scheduled_at for one-shots.
      def recalculate_next_occurrence(record)
        next_at = if record.rrule.present?
          ScheduleCalculator.next_occurrence(record.scheduled_at, record.rrule)
        else
          record.scheduled_at
        end
        record.update_columns(next_occurrence_at: next_at)
      end

      # Resolve `account_id` to an account the acting user may send from, or
      # render 403 and return nil (so callers can `return unless`). Callers
      # assign the returned record to email_account explicitly rather than
      # mass-assigning email_account_id, so a token can't schedule a send from
      # an account in another workspace.
      def sendable_account(account_id)
        account = Current.user.sendable_email_accounts.find_by(id: account_id) if account_id.present?
        return account if account

        render_api_error("no_sendable_account",
                         "You can't send from that email account.", status: :forbidden)
        nil
      end

      def valid_status?(value)
        value.present? && ScheduledEmail.statuses.key?(value)
      end

      def scheduled_email_params
        params.permit(
          :email_template_id, :to_address, :cc_address, :bcc_address,
          :subject, :body, :scheduled_at, :rrule, template_context: {}
        )
      end
    end
  end
end
