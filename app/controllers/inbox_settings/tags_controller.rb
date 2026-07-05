module InboxSettings
  # Tags management panel. Mirrors the queries of the old Settings::TagsController
  # but renders into the modal frame and responds to CRUD with Turbo Streams.
  class TagsController < BaseController
    before_action :set_tag, only: [ :edit, :update, :destroy ]

    def index
      load_tag_groups
    end

    # Flip a tag between visible and hidden. The decision is remembered on the
    # tag (it survives re-syncs because classified labels are never re-classified),
    # so this is how a user overrides an AI/provider hide — or hides a tag they
    # don't want as a chip.
    def toggle_hidden
      @tag = Current.workspace.tags.find(params[:id])
      @tag.update!(hidden: !@tag.hidden?)
      load_tag_groups
      # -> toggle_hidden.turbo_stream.erb
    end

    def new
      @tag = Current.workspace.tags.new
    end

    def create
      @tag = Current.workspace.tags.new(tag_params)
      if @tag.save
        # -> create.turbo_stream.erb
      else
        render_form_errors
      end
    end

    def edit
    end

    def update
      if @tag.external? && provider_fields_changed?
        # Push the rename/recolor to the provider first; only save locally on success.
        begin
          result = @tag.email_account.mail_client.update_label(
            @tag.external_label_id,
            name: tag_params[:name].presence || @tag.name,
            color: tag_params[:color].presence || @tag.color
          )
          unless result.dig("status", "code") == 200 || result["id"].present?
            render turbo_stream: notify_stream(t(".provider_update_failed"), severity: :error)
            return
          end
        rescue => e
          Rails.logger.error("[InboxSettings::Tags] Provider update_label failed: #{e.message}")
          render turbo_stream: notify_stream(t(".provider_update_failed"), severity: :error)
          return
        end
      end

      if @tag.update(tag_params)
        # -> update.turbo_stream.erb
      else
        render_form_errors
      end
    end

    def destroy
      if @tag.name == "security_flagged"
        render turbo_stream: notify_stream(t(".cannot_delete"), severity: :warning)
        return
      end

      if @tag.external?
        begin
          @tag.email_account.mail_client.delete_label(@tag.external_label_id)
        rescue => e
          Rails.logger.error("[InboxSettings::Tags] Provider delete_label failed: #{e.message}")
        end
      end

      @tag.destroy
      # -> destroy.turbo_stream.erb
    end

    private

    # Validation errors re-render the form inside its frame. Done explicitly as a
    # Turbo Stream so it works regardless of the request's format negotiation.
    def render_form_errors
      render turbo_stream: turbo_stream.update(
        "inbox_settings_tag_form",
        partial: "inbox_settings/tags/form",
        locals: { tag: @tag }
      ), status: :unprocessable_entity
    end

    def set_tag
      @tag = Current.workspace.tags.find(params[:id])
    end

    # True when the submitted params would change the name or color — the two
    # fields that need to be mirrored to the provider on external tags.
    def provider_fields_changed?
      (tag_params[:name].present? && tag_params[:name] != @tag.name) ||
        (tag_params[:color].present? && tag_params[:color] != @tag.color)
    end

    # Visible tags, plus the hidden tags grouped by why they're hidden:
    # provider system/category statuses vs everything else (AI low-value + any
    # tag the user hid by hand).
    def load_tag_groups
      tags = Current.workspace.tags
      @visible_tags    = tags.visible.order(:name).to_a
      @hidden_system   = tags.hidden_labels.where(kind: [ :system, :category ]).order(:name).to_a
      @hidden_filtered = tags.hidden_labels.where.not(kind: [ :system, :category ]).order(:name).to_a
      ids = (@visible_tags + @hidden_system + @hidden_filtered).map(&:id)
      @tag_message_counts = EmailMessageTag.where(tag_id: ids).group(:tag_id).count
    end

    def tag_params
      params.require(:tag).permit(:name, :color, :prompt, :group_name)
    end
  end
end
