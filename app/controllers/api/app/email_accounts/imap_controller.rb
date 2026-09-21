# frozen_string_literal: true

module Api
  module App
    module EmailAccounts
      # IMAP account connect + credentials update. Verifies credentials
      # synchronously via Imap::MailClient#verify! before saving (same as the web
      # ImapAccountsController). This blocks the request for the IMAP handshake
      # duration — typically 1-3 s; the client should show a loading state.
      #
      # The endpoint is gated by Features.imap? (returns 404 when IMAP is off).
      class ImapController < BaseController
        before_action :require_imap_enabled

        # POST /api/app/imap_accounts
        def create
          normalized = params.require(:email_address).to_s.strip.downcase
          existing = ::EmailAccount.find_by(email_address: normalized)

          if existing
            # Reconnect only if the acting user manages this account.
            unless existing.imap? && existing.managed_by?(current_user)
              return render_error("already_connected",
                                  "That address is already connected with a different provider or owner.",
                                  status: :unprocessable_entity)
            end
            @account = existing
            @account.assign_attributes(imap_params)
            @account.email_address = normalized
            @account.active = true
          else
            @account = ::EmailAccount.new(provider: :imap, workspace: current_workspace)
            @account.assign_attributes(imap_params)
            @account.email_address = normalized
          end

          verify_credentials || return

          @account.save!
          @account.email_account_users.find_or_create_by!(user: current_user) do |entry|
            entry.owner      = true
            entry.can_read   = true
            entry.can_send   = true
            entry.can_manage = true
          end
          render_data(Api::App::Email::AccountSerializer.new(@account, user: current_user).as_json, status: :created)
        end

        # PATCH /api/app/imap_accounts/:id
        def update
          @account = current_user.readable_email_accounts.find(params[:id])
          unless @account.imap? && @account.managed_by?(current_user)
            return render_error("forbidden", "Only IMAP account managers can update credentials.",
                                status: :forbidden)
          end

          @account.assign_attributes(imap_params)
          verify_credentials || return

          @account.save!
          render_data(Api::App::Email::AccountSerializer.new(@account, user: current_user).as_json)
        end

        private

        def require_imap_enabled
          render_not_found unless ::Features.imap?
        end

        def imap_params
          params.permit(
            :imap_host, :imap_port, :imap_security,
            :smtp_host, :smtp_port, :smtp_security,
            :imap_password
          )
        end

        def verify_credentials
          @account.mail_client.verify!
          true
        rescue => e
          render_error("imap_verification_failed", e.message, status: :unprocessable_entity)
          false
        end
      end
    end
  end
end
