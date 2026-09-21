# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Email signature CRUD + set_default. Signatures are user-scoped.
        class SignaturesController < Api::App::BaseController
          before_action :set_signature, only: %i[show update destroy set_default]

          # GET /api/app/inbox_settings/signatures
          def index
            signatures = current_user.signatures.ordered
            render_data signatures.map { |s| serialize(s) }
          end

          # GET /api/app/inbox_settings/signatures/:id
          def show
            render_data serialize(@signature)
          end

          # POST /api/app/inbox_settings/signatures
          def create
            signature = current_user.signatures.new(signature_params)
            if signature.save
              signature.make_default! if signature.is_default?
              render_data serialize(signature), status: :created
            else
              render_error("invalid", signature.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # PATCH /api/app/inbox_settings/signatures/:id
          def update
            if @signature.update(signature_params)
              @signature.make_default! if @signature.is_default?
              render_data serialize(@signature)
            else
              render_error("invalid", @signature.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # DELETE /api/app/inbox_settings/signatures/:id
          def destroy
            @signature.destroy
            render json: {}, status: :no_content
          end

          # POST /api/app/inbox_settings/signatures/:id/set_default
          def set_default
            @signature.make_default!
            render_data serialize(@signature)
          end

          private

          def set_signature
            @signature = current_user.signatures.find(params[:id])
          end

          def signature_params
            permitted = params.require(:signature).permit(:name, :content, :is_default, email_account_ids: [])
            if permitted[:email_account_ids]
              owned = current_user.email_accounts.pluck(:id).map(&:to_s)
              permitted[:email_account_ids] = permitted[:email_account_ids].select { |id| owned.include?(id.to_s) }
            end
            permitted
          end

          def serialize(signature)
            Api::App::Settings::SignatureSerializer.new(signature).as_json
          end
        end
      end
    end
  end
end
