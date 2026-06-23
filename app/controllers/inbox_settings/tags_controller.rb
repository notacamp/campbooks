module InboxSettings
  # Tags management panel. Mirrors the queries of the old Settings::TagsController
  # but renders into the modal frame and responds to CRUD with Turbo Streams.
  class TagsController < BaseController
    before_action :set_tag, only: [ :edit, :update, :destroy ]

    def index
      @tags = Current.workspace.tags.order(:name).to_a
      @tag_message_counts = EmailMessageTag.where(tag_id: @tags.map(&:id)).group(:tag_id).count
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

    def tag_params
      params.require(:tag).permit(:name, :color, :prompt, :group_name)
    end
  end
end
