# frozen_string_literal: true

module Api
  module App
    module Settings
      # Current-user account profile + preferences.
      # Password change and account deletion require the current password in the
      # request body (same guard as the web controller).
      class AccountController < Api::App::BaseController
        # GET /api/app/account
        def show
          render_data serializer.as_json
        end

        # PATCH /api/app/account  — password change only (name handled by language/prefs)
        def update
          unless current_user.authenticate(params[:current_password].to_s)
            return render_error("wrong_password", "Current password is incorrect.", status: :unprocessable_entity)
          end

          if current_user.update(password_params)
            # Revoke all other sessions on password change (OWASP ASVS 2.2.1).
            current_user.sessions.where.not(id: Current.session&.id).destroy_all
            AuditEvent.log("password_changed", user: current_user, request: request)
            render_data serializer.as_json
          else
            render_record_invalid_errors
          end
        end

        # PATCH /api/app/account/language
        def language
          if current_user.update(params.permit(:locale))
            render_data serializer.as_json
          else
            render_error("invalid", current_user.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/account/compose_preference
        def compose_preference
          if current_user.update(params.permit(:compose_default))
            render_data serializer.as_json
          else
            render_error("invalid", current_user.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/account/writing_style
        def writing_style
          attrs = params.permit(:writing_style).merge(writing_style_updated_at: ::Time.current)
          if current_user.update(attrs)
            render_data serializer.as_json
          else
            render_error("invalid", current_user.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # POST /api/app/account/analyze_writing_style
        def analyze_writing_style
          WritingStyleProfileJob.perform_later(current_user.id)
          render json: { data: { queued: true } }
        end

        # POST /api/app/account/export
        def export
          AuditEvent.log("data_exported", user: current_user, request: request)
          account_export = current_user.account_exports.create!(status: :pending)
          AccountExportJob.perform_later(account_export.id)
          render json: { data: { export_id: account_export.id, status: "pending" } }, status: :accepted
        end

        # GET /api/app/account/export
        def download_export
          account_export = current_user.account_exports.generated.recent.first
          if account_export&.archive&.attached?
            url = rails_blob_url(account_export.archive, disposition: "attachment", only_path: false)
            render json: { data: { url: url, export_id: account_export.id } }
          else
            render_error("export_not_ready", "No export archive is ready yet.", status: :not_found)
          end
        end

        # DELETE /api/app/account
        def destroy
          unless current_user.authenticate(params[:current_password].to_s)
            return render_error("wrong_password", "Current password is incorrect.", status: :unprocessable_entity)
          end

          unless params[:confirm_email].to_s.strip.downcase == current_user.email_address
            return render_error("wrong_email", "Email confirmation does not match.", status: :unprocessable_entity)
          end

          current_user.update!(deletion_requested_at: ::Time.current)
          AuditEvent.log("account_deletion_requested", user: current_user, request: request)
          AccountDeletionJob.perform_later(current_user.id)
          Current.session&.destroy
          render json: {}, status: :no_content
        end

        private

        def serializer
          Api::App::Settings::AccountSerializer.new(current_user)
        end

        def password_params
          params.permit(:password, :password_confirmation)
        end

        def render_record_invalid_errors
          render_error("invalid", current_user.errors.full_messages.to_sentence, status: :unprocessable_entity)
        end
      end
    end
  end
end
