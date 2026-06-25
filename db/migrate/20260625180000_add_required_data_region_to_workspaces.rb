class AddRequiredDataRegionToWorkspaces < ActiveRecord::Migration[8.1]
  def change
    # Per-workspace data-residency policy. NULL = no restriction (the default, so
    # existing workspaces are untouched); "EU" = only EU-region AI providers may be
    # used (Workspace#region_allows?), pausing AI paths that have no EU provider yet.
    add_column :workspaces, :required_data_region, :string
  end
end
