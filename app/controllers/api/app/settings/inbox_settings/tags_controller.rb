# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Tag CRUD + merge + toggle_hidden.
        class TagsController < Api::App::BaseController
          before_action :set_tag, only: %i[show update destroy toggle_hidden commit_merge]

          # GET /api/app/inbox_settings/tags
          def index
            tags = current_workspace.tags.order(:name)
            counts = EmailMessageTag.where(tag_id: tags.map(&:id)).group(:tag_id).count

            render_data({
              visible: tags.visible.map { |t| serialize(t, counts[t.id]) },
              hidden_system: tags.hidden_labels.where(kind: %i[system category]).order(:name)
                                 .map { |t| serialize(t, counts[t.id]) },
              hidden_filtered: tags.hidden_labels.where.not(kind: %i[system category]).order(:name)
                                   .map { |t| serialize(t, counts[t.id]) },
              pending_review_count: LabelImportDecision.for_workspace(current_workspace).pending_review.count
            })
          end

          # GET /api/app/inbox_settings/tags/:id
          def show
            render_data serialize(@tag)
          end

          # POST /api/app/inbox_settings/tags
          def create
            tag = current_workspace.tags.new(tag_params)
            if tag.save
              render_data serialize(tag), status: :created
            else
              render_error("invalid", tag.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # PATCH /api/app/inbox_settings/tags/:id
          def update
            if @tag.update(tag_params)
              render_data serialize(@tag)
            else
              render_error("invalid", @tag.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end

          # DELETE /api/app/inbox_settings/tags/:id
          def destroy
            if @tag.name == "security_flagged"
              return render_error("invalid", "This tag cannot be deleted.", status: :unprocessable_entity)
            end

            @tag.destroy
            render json: {}, status: :no_content
          end

          # PATCH /api/app/inbox_settings/tags/:id/toggle_hidden
          def toggle_hidden
            @tag.update!(hidden: !@tag.hidden?)
            render_data serialize(@tag)
          end

          # POST /api/app/inbox_settings/tags/:id/merge
          def commit_merge
            target = current_workspace.tags.find(params[:into_tag_id])
            Tags::MergeService.new(source: @tag, target: target).merge!
            render json: { data: { merged: true, into_tag_id: target.id } }
          rescue Tags::MergeService::MergeError => e
            render_error("invalid", e.message, status: :unprocessable_entity)
          end

          private

          def set_tag
            @tag = current_workspace.tags.find(params[:id])
          end

          # Accept both flat params (API clients) and tag[*] nesting (compatibility).
          def tag_params
            if params[:tag].present?
              params.require(:tag).permit(:name, :color, :prompt, :group_name)
            else
              params.permit(:name, :color, :prompt, :group_name)
            end
          end

          def serialize(tag, message_count = nil)
            Api::App::Settings::TagSerializer.new(tag, message_count: message_count).as_json
          end
        end
      end
    end
  end
end
