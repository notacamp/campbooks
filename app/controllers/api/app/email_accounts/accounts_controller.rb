# frozen_string_literal: true

module Api
  module App
    module EmailAccounts
      # Connected email account management — list, update settings, disconnect,
      # sharing panel. The OAuth connect kickoff is NOT here: it remains a
      # browser redirect (see oauth_controller.rb which returns the authorize URL).
      # Permissions mirror the web EmailAccountsController: rename/settings = manager,
      # sharing panel + disconnect = owner.
      class AccountsController < BaseController
        before_action :set_account, only: %i[update destroy sharing_show sharing_update popover]

        # GET /api/app/email_accounts
        def index
          accounts = current_user.readable_email_accounts.active.includes(:email_account_users).order(:id)
          render_data(accounts.map { |a| Api::App::Email::AccountSerializer.new(a, user: current_user).as_json })
        end

        # PATCH /api/app/email_accounts/:id
        def update
          unless @account.managed_by?(current_user)
            return render_error("forbidden", "Only managers can update account settings.", status: :forbidden)
          end

          permitted = params.permit(:name, :color)
          if @account.update(permitted)
            render_data(Api::App::Email::AccountSerializer.new(@account, user: current_user).as_json)
          else
            render_error("invalid", @account.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # DELETE /api/app/email_accounts/:id
        def destroy
          # 404-not-403 rule: don't reveal to a sharee that they lack permission.
          # Raise NotFound so it looks identical to a missing account.
          raise ::ActiveRecord::RecordNotFound unless @account.owned_by?(current_user)

          ::Events.publish("email_account.disconnected",
                           subject: @account,
                           payload: { "email_address" => @account.email_address, "provider" => @account.provider })
          @account.update_columns(active: false)
          ::EmailAccountRemovalJob.perform_later(@account.id)
          head :no_content
        end

        # GET /api/app/email_accounts/:id/sharing
        def sharing_show
          # Only owners see the sharing panel (404-not-403 rule — don't reveal
          # it exists to non-owners either).
          raise ::ActiveRecord::RecordNotFound unless @account.owned_by?(current_user)

          members = @account.email_account_users.includes(:user).map do |eau|
            {
              user_id: eau.user_id,
              email: eau.user&.email_address,
              name: eau.user&.name,
              can_read: eau.can_read,
              can_send: eau.can_send,
              can_manage: eau.can_manage,
              owner: eau.owner
            }
          end
          render_data({
            account: Api::App::Email::AccountSerializer.new(@account, user: current_user).as_json,
            members: members
          })
        end

        # PATCH /api/app/email_accounts/:id/sharing
        def sharing_update
          unless @account.owned_by?(current_user)
            return render_error("forbidden", "Only the account owner can change sharing.", status: :forbidden)
          end

          target_user = current_workspace.users.find_by(id: params.require(:user_id))
          raise ::ActiveRecord::RecordNotFound unless target_user

          eau = @account.email_account_users.find_or_initialize_by(user: target_user)
          eau.assign_attributes(
            can_read:   params.key?(:can_read)   ? params[:can_read]   : eau.can_read,
            can_send:   params.key?(:can_send)   ? params[:can_send]   : eau.can_send,
            can_manage: params.key?(:can_manage) ? params[:can_manage] : eau.can_manage
          )
          eau.save!
          head :ok
        end

        # GET /api/app/email_accounts/:id/popover
        def popover
          msg_count = @account.email_messages.count
          render_data({
            account: Api::App::Email::AccountSerializer.new(@account, user: current_user).as_json,
            message_count: msg_count
          })
        end

        private

        def set_account
          @account = current_user.readable_email_accounts.find(params[:id])
        end
      end
    end
  end
end
