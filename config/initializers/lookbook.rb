# frozen_string_literal: true

return unless defined?(Lookbook)

Lookbook.configure do |config|
  config.component_paths += [ Rails.root.join("app", "components") ]
  # Use a minimal layout; the app layout calls authenticated? which isn't
  # available in the isolated preview context.
  config.preview_layout = "lookbook_preview"
  # Most previews live under test/components/previews (ViewComponent's default);
  # also surface the few kept under spec/ so none are silently unreachable.
  config.preview_paths += [ Rails.root.join("spec", "components", "previews") ]
end
