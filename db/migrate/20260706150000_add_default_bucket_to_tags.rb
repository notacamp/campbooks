# frozen_string_literal: true

# Marks a tag as one of the built-in default group tags (Notifications /
# Newsletters & promos / Social / Updates). The value is the stable key the
# category->tag bridge and provisioning look up by, so the user is free to
# rename, recolor, or regroup the tag without breaking either. Only ever set on
# four tags per workspace; NULL for every ordinary tag.
class AddDefaultBucketToTags < ActiveRecord::Migration[8.1]
  def change
    add_column :tags, :default_bucket, :string

    add_index :tags, %i[workspace_id default_bucket],
              name: "idx_tags_on_workspace_and_default_bucket",
              unique: true,
              where: "default_bucket IS NOT NULL"
  end
end
