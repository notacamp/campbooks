class CreateEvents < ActiveRecord::Migration[8.1]
  def change
    create_table :events do |t|
      t.references :workspace, null: false, foreign_key: true
      # Dotted event type, e.g. "document.approved". Not restricted to a fixed
      # set — Events::Registry catalogs the known ones for the UI, but any string
      # may be published (custom workflow emit_event, future external sources).
      t.string :name, null: false

      # The record the event is about (optional).
      t.references :subject, polymorphic: true
      # Who caused it: a User, a Workflow, … or nil for system-generated events.
      t.references :actor, polymorphic: true
      # Causation chain: the event that ultimately led to this one being emitted
      # (set when a workflow's emit_event action fires). Powers the audit trail
      # and, with `depth`, bounds runaway emit→trigger→emit loops. Nullify on
      # delete so retention pruning / workspace teardown can remove a parent
      # event without tripping the self-referential FK on a surviving child.
      t.references :caused_by_event, foreign_key: { to_table: :events, on_delete: :nullify }
      t.integer :depth, null: false, default: 0

      # Arbitrary structured data, exposed to Liquid in workflow steps.
      t.jsonb :payload, null: false, default: {}

      t.datetime :occurred_at, null: false
      t.timestamps
    end

    # Workflow matching (by name) + the filtered activity feed.
    add_index :events, [ :workspace_id, :name, :occurred_at ]
    # Default activity-feed ordering.
    add_index :events, [ :workspace_id, :occurred_at ]
    # The polymorphic subject/actor/caused_by_event indexes are created by
    # t.references above (used for "events for this record" lookups).
  end
end
