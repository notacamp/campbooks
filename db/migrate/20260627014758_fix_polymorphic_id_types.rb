# frozen_string_literal: true

# Fix polymorphic *_id columns: text → uuid (PostgreSQL can't join uuid = text).
class FixPolymorphicIdTypes < ActiveRecord::Migration[8.1]
  def up
    change_column :pipeline_memberships, :item_id, :uuid, using: "item_id::text::uuid"
  end

  def down
    raise ActiveRecord::IrreversibleMigration
  end
end
