# frozen_string_literal: true

class Settings::SetupTemplateController < Settings::BaseController
  before_action :set_workspace

  def show
    @chosen_key   = @workspace.setting("setup_template")
    @chosen        = Onboarding::Templates.find(@chosen_key) if @chosen_key
    @templates     = Onboarding::Templates.all
    @applied_tags  = applied_tags
    @applied_types = applied_doc_types
    @module_keys   = %w[calendar files contacts organizations activity]
  end

  # PATCH /settings/setup_template — switch or re-apply a template.
  # Non-destructive: only adds, never removes.
  def update
    key = params[:template_key].to_s

    unless Onboarding::Templates.keys.include?(key)
      redirect_to settings_setup_template_path, error: t(".invalid_template")
      return
    end

    Onboarding::TemplateApplier.new(@workspace, key).apply!

    redirect_to settings_setup_template_path, success: t(".applied", name: template_name_for(key))
  end

  # PATCH /settings/setup_template/modules — toggle individual module visibility.
  def update_modules
    # The checkbox form sends only checked keys. Build the full visibility map
    # from all known module keys so unchecked ones resolve to false.
    known_keys = Onboarding::Templates::CATALOG.flat_map { |t| t[:module_visibility].keys }.uniq
    checked_keys = params[:module_visibility].respond_to?(:keys) ? params[:module_visibility].keys.map(&:to_s) : []
    new_visibility = known_keys.index_with { |k| checked_keys.include?(k) }

    # Merge with existing (don't drop keys set by other code paths).
    merged = (@workspace.settings["module_visibility"] || {}).merge(new_visibility)
    @workspace.settings["module_visibility"] = merged
    @workspace.save!

    redirect_to settings_setup_template_path, success: t(".modules_saved")
  end

  private

  def set_workspace
    @workspace = Current.workspace || current_user&.workspace
  end

  # Tags provisioned by the currently active template (if any).
  def applied_tags
    return [] unless @chosen
    names = @chosen[:tags].map { |t| t[:name] }
    @workspace.tags.where(name: names).order(:name)
  end

  # Document types provisioned by the currently active template (if any).
  def applied_doc_types
    return [] unless @chosen
    names = @chosen[:document_types].map { |t| t[:name] }
    @workspace.document_types.where(name: names).order(:name)
  end

  def template_name_for(key)
    I18n.t("onboarding.steps.template.templates.#{key}.name", default: key.humanize)
  end
end
