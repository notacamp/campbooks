# frozen_string_literal: true

module Api
  module App
    module Calendar
      # Calendar account management: rename, disconnect, sharing panel, and
      # on-demand provider calendar list refresh.
      class CalendarAccountsController < Api::App::BaseController
        before_action :set_account, only: %i[update destroy sharing]

        # PATCH /api/app/calendar_accounts/:id
        # Two sub-operations on the same endpoint (mirrors the web controller):
        #   - calendar_account[name] present → rename (requires manage)
        #   - user_email + role/remove       → access change (requires owner)
        def update
          if params[:calendar_account].present?
            return require_manage_or_403! unless @account.managed_by?(current_user)
            update_account_settings
          else
            return require_owner_or_403! unless @account.owned_by?(current_user)
            update_user_permissions
          end
        end

        # DELETE /api/app/calendar_accounts/:id — disconnect the account (owner only).
        def destroy
          return require_owner_or_403! unless @account.owned_by?(current_user)

          @account.deactivate!
          Accounts::TokenRevoker.revoke_if_unshared(@account)
          head :no_content
        end

        # GET /api/app/calendar_accounts/:id/sharing — sharing panel (owner only).
        def sharing
          return require_owner_or_403! unless @account.owned_by?(current_user)

          members = @account.calendar_account_users.includes(:user).to_a
                            .sort_by { |m| [ m.owner? ? 0 : 1, m.user.name.to_s.downcase ] }
          member_ids = members.map(&:user_id)
          addable = current_workspace.users.where.not(id: member_ids).order(:name)

          render_data({
            members: members.map { |m| member_as_json(m) },
            addable_users: addable.map { |u| { id: u.id, name: u.name, email: u.email_address } },
            roles: ::CalendarAccountUser::ROLES
          })
        end

        # POST /api/app/calendar_accounts/refresh — re-pull provider calendar list.
        def refresh
          account = current_user.manageable_calendar_accounts.find(params[:calendar_account_id])
          unless account.managed_by?(current_user)
            return render_error("not_permitted",
                                "You do not have permission to refresh this calendar account.",
                                status: :forbidden)
          end

          ::CalendarScanJob.perform_later(account.id, "full")
          render_data({ queued: true })
        end

        private

        def set_account
          @account = current_user.calendar_accounts.find(params[:id])
        end

        def update_account_settings
          @account.update!(calendar_account_params)
          render_data(serialize_account(@account))
        end

        def update_user_permissions
          target = current_workspace.users.find_by(email_address: params[:user_email]&.strip&.downcase)
          return render_error("user_not_found", "User not found in this workspace.", status: :not_found) unless target

          entry = @account.calendar_account_users.find_or_initialize_by(user: target)

          if params[:remove] == "true"
            return render_error("owner_protected", "Owner access cannot be removed.", status: :unprocessable_entity) if entry.owner?
            entry.destroy!
            return render_data(serialize_account(@account))
          end

          return render_error("owner_role_fixed", "The owner's role cannot be changed.", status: :unprocessable_entity) if entry.owner?

          unless ::CalendarAccountUser::ROLES.include?(params[:role])
            return render_error("invalid_role", "Role must be one of: #{CalendarAccountUser::ROLES.join(', ')}.",
                                status: :unprocessable_entity)
          end

          entry.role = params[:role]
          entry.save!
          render_data(serialize_account(@account))
        end

        def require_manage_or_403!
          render_error("not_permitted",
                       "You need manager access to update this calendar account.",
                       status: :forbidden)
        end

        def require_owner_or_403!
          render_error("owner_only",
                       "Only the owner can perform this action.",
                       status: :forbidden)
        end

        def calendar_account_params
          params.require(:calendar_account).permit(:name)
        end

        def serialize_account(account)
          Api::App::Calendar::CalendarAccountSerializer.new(
            account,
            user: current_user,
            managed_ids: current_user.manageable_calendar_accounts.pluck(:id)
          ).as_json
        end

        def member_as_json(member)
          {
            id: member.id,
            user_id: member.user_id,
            name: member.user.name,
            email: member.user.email_address,
            role: member.role,
            owner: member.owner?
          }
        end
      end
    end
  end
end
