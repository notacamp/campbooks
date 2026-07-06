# frozen_string_literal: true

class Settings::SetupTemplateController < Settings::BaseController
  before_action :set_workspace

  def show
    # Read the array of active template keys (new format).
    @chosen_keys   = Array(@workspace.settings["setup_templates"]).compact
    @chosen_templates = @chosen_keys.filter_map { |k| Onboarding::Templates.find(k) }
    @templates     = Onboarding::Templates.all
    @applied_tags  = applied_tags
    @applied_types = applied_doc_types
    @module_keys   = %w[calendar files contacts organizations activity]
  end

  # PATCH /settings/setup_template — apply (or add to) one or more templates.
  # Non-destructive: only adds, never removes.
  def update
    keys = Array(params[:template_keys]).map(&:to_s).select { |k| Onboarding::Templates.keys.include?(k) }

    if keys.empty?
      redirect_to settings_setup_template_path, error: t(".invalid_template")
      return
    end

    Onboarding::TemplateApplier.new(@workspace, keys).apply!

    names = keys.map { |k| template_name_for(k) }.to_sentence
    redirect_to settings_setup_template_path, success: t(".applied", name: names)
  end

  # PATCH /settings/setup_template/modules — toggle individual module visibility.
  def update_modules
    # The checkbox form sends only checked keys. Build the full visibility map
    # from all known module keys so unchecked ones resolve to false.
    known_keys   = Onboarding::Templates::CATALOG.flat_map { |t| t[:module_visibility].keys }.uniq
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

  # Tags provisioned by any of the currently active templates.
  def applied_tags
    return [] if @chosen_templates.empty?

    names = @chosen_templates.flat_map { |t| t[:tags].map { |tag| tag[:name] } }.uniq
    @workspace.tags.where(name: names).order(:name)
  end

  # Document types provisioned by any of the currently active templates.
  def applied_doc_types
    return [] if @chosen_templates.empty?

    names = @chosen_templates.flat_map { |t| t[:document_types].map { |dt| dt[:name] } }.uniq
    @workspace.document_types.where(name: names).order(:name)
  end

  def template_name_for(key)
    I18n.t("onboarding.steps.template.templates.#{key}.name", default: key.humanize)
  end
end
