# frozen_string_literal: true

module Api
  module V1
    # Attach/detach a workspace tag on an email message. The email is gated by
    # EmailMessage.accessible_to (the acting user must be able to read the account).
    # Tags are looked up, never created here — create tags in the app first.
    class EmailTagsController < BaseController
      before_action -> { doorkeeper_authorize! :"tags:write" }, only: [ :create, :destroy ]
      before_action :set_email

      # POST /api/v1/emails/:email_id/tags  body: { tag_id: } or { name: }
      def create
        tag =
          if params[:tag_id].present?
            Current.workspace.tags.find(params[:tag_id])
          elsif params[:name].present?
            Current.workspace.tags.find_by!("LOWER(name) = ?", params[:name].to_s.downcase.strip)
          else
            return render_api_error("missing_parameter", "Provide tag_id or name.", status: :bad_request)
          end

        @email.tags << tag unless @email.tags.include?(tag)
        render_data(TagSerializer.new(tag).as_json, status: :created)
      end

      # DELETE /api/v1/emails/:email_id/tags/:id  (id = tag id; 404 if not on the email)
      def destroy
        tag = @email.tags.find(params[:id])
        @email.tags.delete(tag)
        head :no_content
      end

      private

      def set_email
        @email = EmailMessage.accessible_to(current_user).find(params[:email_id])
      end
    end
  end
end
