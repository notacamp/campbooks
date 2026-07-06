module InboxSettings
  # The Groups panel: create a group, pick which tags belong to it and/or add
  # matching rules (sender, organization, document type, search query), and the
  # inbox collapses their mail into that group's row. A group is the union of
  # its tagged threads and its rule-based threads, so creating a rules-only
  # group (no tags) is fully supported.
  #
  # Group identity is a name string, not an id: tags carry group_name; rules
  # carry group_name. All collection routes use form/query params so names
  # containing spaces or "&" (e.g. "Newsletters & promos") round-trip cleanly.
  class TagGroupsController < BaseController
    def index
      load_groups
    end

    def new
      @group_name = nil
      @member_ids = []
      @rules = []
      load_pickable_tags
      load_rule_options
      render :form
    end

    def edit
      @group_name = existing_group_names.include?(params[:group].to_s) ? params[:group].to_s : nil
      return redirect_to inbox_settings_tag_groups_path if @group_name.blank?

      @member_ids = Current.workspace.tags.where(group_name: @group_name).pluck(:id)
      @rules = Current.workspace.inbox_group_rules.for_group(@group_name).ordered.to_a
      load_pickable_tags
      load_rule_options
      render :form
    end

    def create
      ActiveRecord::Base.transaction do
        apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
        save_rules!(name: params[:name], rules_params: parsed_rules_params)
      end
      redirect_to inbox_settings_tag_groups_path
    end

    def update
      original = params[:original_name].to_s
      ActiveRecord::Base.transaction do
        if original.present?
          Current.workspace.tags.where(group_name: original)
                 .update_all(group_name: nil, updated_at: Time.current)
          Current.workspace.inbox_group_rules.where(group_name: original).destroy_all
        end
        apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
        save_rules!(name: params[:name], rules_params: parsed_rules_params)
      end
      redirect_to inbox_settings_tag_groups_path
    end

    def destroy
      name = params[:group].to_s
      ActiveRecord::Base.transaction do
        Current.workspace.tags.where(group_name: name)
               .update_all(group_name: nil, updated_at: Time.current)
        Current.workspace.inbox_group_rules.where(group_name: name).destroy_all
      end
      redirect_to inbox_settings_tag_groups_path
    end

    private

    # Set group_name on the selected tags. A tag can belong to only one group at
    # a time; selecting a tag from another group moves it here. Unlike before,
    # an empty tag selection is now permitted (a group may be rules-only).
    def apply_membership!(name:, tag_ids:)
      name = name.to_s.strip
      ids  = Array(tag_ids).reject(&:blank?)
      return if name.blank?

      Current.workspace.tags.where(id: ids).update_all(group_name: name, updated_at: Time.current)
    end

    # Replace all rules for this group with the submitted set.
    def save_rules!(name:, rules_params:)
      name = name.to_s.strip
      return if name.blank?

      Current.workspace.inbox_group_rules.where(group_name: name).destroy_all

      rules_params.each do |rule|
        rule_type = rule[:rule_type].to_s
        value     = rule[:value].to_s.strip
        next if rule_type.blank? || value.blank?
        next unless InboxGroupRule::RULE_TYPES.include?(rule_type)

        Current.workspace.inbox_group_rules.create!(
          group_name: name,
          rule_type:  rule_type,
          value:      value
        )
      end
    end

    # Parse the rules[] array from form params.  Each element is expected to be
    # a hash (or ActionController::Parameters) with :rule_type and :value.
    def parsed_rules_params
      Array(params[:rules]).filter_map do |r|
        next unless r.is_a?(Hash) || r.respond_to?(:to_unsafe_h)
        h = r.respond_to?(:to_unsafe_h) ? r.to_unsafe_h : r
        { rule_type: h["rule_type"].to_s, value: h["value"].to_s }
      end
    end

    # All group names that currently exist for this workspace (from tags or rules).
    def existing_group_names
      tag_names  = Current.workspace.tags.visible.grouped.pluck(:group_name).uniq
      rule_names = Current.workspace.inbox_group_rules.pluck(:group_name).uniq
      (tag_names + rule_names).uniq
    end

    def groups_scope
      Current.workspace.tags.visible.grouped
    end

    # Groups as [name, member tags] pairs, plus rule counts and collapsed thread
    # counts (same engine the inbox uses so numbers agree).
    def load_groups
      tag_groups_by_name = groups_scope.by_name.group_by(&:group_name).sort_by { |name, _| name.downcase }
      rule_names = Current.workspace.inbox_group_rules.pluck(:group_name).uniq
      all_names  = (tag_groups_by_name.map(&:first) + rule_names).uniq.sort

      @groups = all_names.map do |name|
        tags  = tag_groups_by_name.to_h[name] || []
        rules = Current.workspace.inbox_group_rules.for_group(name).ordered.to_a
        [ name, tags, rules ]
      end

      account_ids = Current.user.readable_email_accounts.pluck(:id)
      service     = Emails::TagGroups.new(Current.workspace, account_ids)
      @group_thread_counts = @groups.to_h do |name, _tags, _rules|
        [ name, service.group_scope(name)&.count || 0 ]
      end

      # Preload display names for org and document-type rules so the index
      # view can render natural-language sentences without N+1 queries.
      all_rules    = @groups.flat_map { |_, _, rules| rules }
      org_ids      = all_rules.select { |r| r.rule_type == "organization" }.map(&:value).uniq
      dt_ids       = all_rules.select { |r| r.rule_type == "document_type" }.map(&:value).uniq
      @org_names      = org_ids.any? ? Organization.where(id: org_ids).pluck(:id, :name).to_h.transform_keys(&:to_s)   : {}
      @doctype_names  = dt_ids.any?  ? DocumentType.where(id: dt_ids).pluck(:id, :name).to_h.transform_keys(&:to_s)    : {}
    end

    def load_pickable_tags
      @pickable_tags = Current.workspace.tags.visible.by_name.to_a
    end

    def load_rule_options
      @organizations   = Current.workspace.organizations.ordered.to_a
      @document_types  = Current.workspace.document_types.order(:name).to_a
    end
  end
end
