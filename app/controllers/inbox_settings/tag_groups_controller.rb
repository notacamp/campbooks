module InboxSettings
  # The Groups panel: create a group, pick which tags belong to it, and the
  # inbox collapses their mail into that group's row. A group is nothing more
  # than the set of tags sharing a `group_name`, so create/rename/dissolve just
  # rewrite that column across the workspace's tags — dissolving a group never
  # touches the tags themselves. Group names travel as params, never as path
  # segments ("Newsletters & promos" has spaces and an ampersand).
  class TagGroupsController < BaseController
    def index
      load_groups
    end

    def new
      @group_name = nil
      @member_ids = []
      load_pickable_tags
      render :form
    end

    def edit
      @group_name = groups_scope.where(group_name: params[:group]).pick(:group_name)
      return redirect_to inbox_settings_tag_groups_path if @group_name.blank?

      @member_ids = Current.workspace.tags.where(group_name: @group_name).pluck(:id)
      load_pickable_tags
      render :form
    end

    def create
      apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
      redirect_to inbox_settings_tag_groups_path
    end

    def update
      original = params[:original_name].to_s
      Tag.transaction do
        if original.present?
          Current.workspace.tags.where(group_name: original)
                 .update_all(group_name: nil, updated_at: Time.current)
        end
        apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
      end
      redirect_to inbox_settings_tag_groups_path
    end

    def destroy
      Current.workspace.tags.where(group_name: params[:group].to_s)
             .update_all(group_name: nil, updated_at: Time.current)
      redirect_to inbox_settings_tag_groups_path
    end

    private

    # Set `name` on the selected tags. Selecting a tag that already belongs to
    # another group moves it here (a tag has exactly one group). A blank name or
    # an empty selection is a no-op — the form requires the name client-side,
    # and a group only exists through its member tags.
    def apply_membership!(name:, tag_ids:)
      name = name.to_s.strip
      ids = Array(tag_ids).reject(&:blank?)
      return if name.blank? || ids.empty?

      Current.workspace.tags.where(id: ids).update_all(group_name: name, updated_at: Time.current)
    end

    def groups_scope
      Current.workspace.tags.visible.grouped
    end

    # Groups as [name, member tags] pairs, plus how many threads each group's
    # row currently collapses (same engine the inbox uses, so numbers agree).
    def load_groups
      @groups = groups_scope.by_name.group_by(&:group_name).sort_by { |name, _| name.downcase }
      account_ids = Current.user.readable_email_accounts.pluck(:id)
      service = Emails::TagGroups.new(Current.workspace, account_ids)
      @group_thread_counts = @groups.to_h do |name, _tags|
        [ name, service.group_scope(name)&.count || 0 ]
      end
    end

    def load_pickable_tags
      @pickable_tags = Current.workspace.tags.visible.by_name.to_a
    end
  end
end
