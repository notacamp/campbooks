class CreateAttentionWeights < ActiveRecord::Migration[8.1]
  def change
    create_table :attention_weights, id: :uuid, default: -> { "gen_random_uuid()" } do |t|
      t.references :user, type: :uuid, null: false, foreign_key: true, index: false
      t.references :workspace, type: :uuid, null: false, foreign_key: true
      t.string :subject_type, null: false
      t.uuid :subject_id, null: false
      t.float :weight, null: false, default: 0.3
      t.float :confidence, null: false, default: 0.0
      t.float :raw_score, null: false, default: 0.0
      t.jsonb :reasons, null: false, default: []
      t.jsonb :evidence, null: false, default: {}
      t.datetime :last_activity_at
      t.datetime :computed_at, null: false
      t.timestamps
    end
    add_index :attention_weights, %i[user_id subject_type subject_id], unique: true, name: "index_attention_weights_on_user_subject"
    add_index :attention_weights, %i[user_id weight], order: { weight: :desc }, name: "index_attention_weights_on_user_weight"
    add_index :attention_weights, %i[subject_type subject_id], name: "index_attention_weights_on_subject"
  end
end
