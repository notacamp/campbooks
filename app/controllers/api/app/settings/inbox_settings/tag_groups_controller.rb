# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Inbox group (stream) CRUD. Groups are name-keyed, not id-keyed.
        # Tags carry group_name; InboxGroupRules carry group_name.
        class TagGroupsController < Api::App::BaseController
          # GET /api/app/inbox_settings/tag_groups
          def index
            render_data groups_data
          end

          # POST /api/app/inbox_settings/tag_groups
          def create
            ActiveRecord::Base.transaction do
              apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
              save_rules!(name: params[:name], rules_params: parsed_rules_params)
            end
            render json: { data: { name: params[:name] } }, status: :created
          rescue ActiveRecord::RecordInvalid => e
            render_error("invalid", e.message, status: :unprocessable_entity)
          end

          # PATCH /api/app/inbox_settings/tag_groups/:id  (id = URL-encoded group name)
          def update
            original = params[:original_name].to_s
            ActiveRecord::Base.transaction do
              if original.present?
                current_workspace.tags.where(group_name: original)
                                 .update_all(group_name: nil, updated_at: ::Time.current)
                current_workspace.inbox_group_rules.where(group_name: original).destroy_all
              end
              apply_membership!(name: params[:name], tag_ids: params[:tag_ids])
              save_rules!(name: params[:name], rules_params: parsed_rules_params)
            end
            render json: { data: { name: params[:name] } }
          rescue ActiveRecord::RecordInvalid => e
            render_error("invalid", e.message, status: :unprocessable_entity)
          end

          # DELETE /api/app/inbox_settings/tag_groups/:id  (id = URL-encoded group name)
          def destroy
            name = params[:id].to_s
            ActiveRecord::Base.transaction do
              current_workspace.tags.where(group_name: name)
                               .update_all(group_name: nil, updated_at: ::Time.current)
              current_workspace.inbox_group_rules.where(group_name: name).destroy_all
            end
            render json: {}, status: :no_content
          end

          private

          def groups_data
            tag_groups_by_name = current_workspace.tags.visible.grouped.by_name.group_by(&:group_name)
            rule_names = current_workspace.inbox_group_rules.pluck(:group_name).uniq
            all_names  = (tag_groups_by_name.keys + rule_names).compact.reject { |n| n.to_s.strip.empty? }.uniq.sort

            all_names.map do |name|
              tags  = tag_groups_by_name[name] || []
              rules = current_workspace.inbox_group_rules.for_group(name).ordered.to_a
              {
                name: name,
                tag_ids: tags.map(&:id),
                tag_names: tags.map(&:name),
                rules: rules.map { |r| { rule_type: r.rule_type, value: r.value } }
              }
            end
          end

          def apply_membership!(name:, tag_ids:)
            name = name.to_s.strip
            ids  = Array(tag_ids).reject(&:blank?)
            return if name.blank?

            current_workspace.tags.where(id: ids)
                             .update_all(group_name: name, updated_at: ::Time.current)
          end

          def save_rules!(name:, rules_params:)
            name = name.to_s.strip
            return if name.blank?

            current_workspace.inbox_group_rules.where(group_name: name).destroy_all
            rules_params.each do |rule|
              rule_type = rule[:rule_type].to_s
              value     = rule[:value].to_s.strip
              next if rule_type.blank? || value.blank?
              next unless InboxGroupRule::RULE_TYPES.include?(rule_type)

              current_workspace.inbox_group_rules.create!(
                group_name: name,
                rule_type:  rule_type,
                value:      value
              )
            end
          end

          def parsed_rules_params
            raw = params[:rules]
            raw = raw.to_unsafe_h if raw.respond_to?(:to_unsafe_h)
            items = raw.is_a?(Hash) ? raw.values : Array(raw)
            items.filter_map do |r|
              r = r.to_unsafe_h if r.respond_to?(:to_unsafe_h)
              next unless r.is_a?(Hash)

              { rule_type: r["rule_type"].to_s, value: r["value"].to_s }
            end
          end
        end
      end
    end
  end
end
