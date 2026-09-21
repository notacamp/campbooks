# frozen_string_literal: true

module Api
  module App
    module Email
      # Add / remove tags on an email message. Mirrors the web
      # EmailMessageTagsController and v1 EmailTagsController. Tags are looked up
      # by id or name (case-insensitive) — never auto-created here.
      class TagsController < BaseController
        before_action :set_message

        # POST /api/app/email_messages/:email_message_id/tags
        # body: { tag_id: } or { name: }
        def create
          tag = find_tag
          return unless tag

          @message.tags << tag unless @message.tags.include?(tag)
          render_data({ id: tag.id, name: tag.name }, status: :created)
        end

        # DELETE /api/app/email_messages/:email_message_id/tags/:tag_id
        def destroy
          tag = @message.tags.find_by(id: params[:tag_id])
          raise ::ActiveRecord::RecordNotFound unless tag

          @message.tags.delete(tag)
          head :no_content
        end

        private

        def set_message
          @message = ::EmailMessage.accessible_to(current_user).find(params[:email_message_id])
        end

        def find_tag
          if params[:tag_id].present?
            current_workspace.tags.find(params[:tag_id])
          elsif params[:name].present?
            tag = current_workspace.tags.find_by("LOWER(name) = ?", params[:name].to_s.downcase.strip)
            render_not_found && nil unless tag
            tag
          else
            render_error("missing_parameter", "Provide tag_id or name.", status: :bad_request)
            nil
          end
        end
      end
    end
  end
end
