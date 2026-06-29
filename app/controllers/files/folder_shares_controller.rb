module Files
  # Per-folder sharing panel (Files Phase 3) — restrict a folder and manage who can
  # access it. Mirrors EmailAccountsController#update_user_permissions: one PATCH
  # endpoint branches on the params (restricted toggle / add-or-update / remove).
  # Manager-gated; 404 (not 403) for folders the user can't manage.
  class FolderSharesController < ApplicationController
    before_action :set_folder
    before_action :require_manage!

    # Rendered into a dialog's turbo-frame on the folder view.
    def show
      load_panel
      render layout: false
    end

    def update
      if params.key?(:restricted)
        toggle_restricted
      elsif params[:remove].present?
        remove_member
      else
        add_or_update_member
      end

      redirect_to files_folder_share_path(@folder)
    end

    private

    def set_folder
      @folder = Current.workspace.mail_folders.find(params[:folder_id])
    end

    def require_manage!
      head :not_found unless @folder.manageable_by?(Current.user)
    end

    def load_panel
      @members = @folder.mail_folder_users.includes(:user).to_a
        .sort_by { |m| [ m.owner? ? 0 : 1, m.user.name.to_s.downcase ] }
      @addable_users = Current.workspace.users.where.not(id: @members.map(&:user_id)).order(:name)
    end

    def toggle_restricted
      restrict = ActiveModel::Type::Boolean.new.cast(params[:restricted])
      @folder.update!(restricted: restrict)

      if restrict
        # Whoever locks the folder becomes its owner so it always has a manager
        # (an admin keeps access either way via the admin bypass).
        @folder.mail_folder_users.find_or_create_by!(user: Current.user) do |m|
          m.owner = true
          m.role = "manager"
        end
        Events.publish("folder.restricted", subject: @folder, payload: { "name" => @folder.name })
      else
        Events.publish("folder.unrestricted", subject: @folder, payload: { "name" => @folder.name })
      end
    end

    def add_or_update_member
      user = Current.workspace.users.find_by(email_address: params[:user_email])
      return if user.nil?

      entry = @folder.mail_folder_users.find_or_initialize_by(user: user)
      return if entry.owner? # the owner's role isn't reassignable here

      entry.role = params[:role]
      entry.save!
      Events.publish("folder.shared", subject: @folder,
        payload: { "name" => @folder.name, "member" => user.email_address })
    end

    def remove_member
      user = Current.workspace.users.find_by(email_address: params[:user_email])
      entry = @folder.mail_folder_users.find_by(user: user)
      return if entry.nil? || entry.owner?

      entry.destroy
      Events.publish("folder.unshared", subject: @folder,
        payload: { "name" => @folder.name, "member" => user.email_address })
    end
  end
end
