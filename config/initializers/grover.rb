# Grover renders HTML -> PDF via headless Chromium for the Document Templates
# feature. The browser binary is resolved at runtime from PUPPETEER_EXECUTABLE_PATH
# (set in the Dockerfile); in the container Chromium runs as a non-root user and
# needs --no-sandbox, which the JS driver enables when GROVER_NO_SANDBOX=true.
#
# allow_file_uris / allow_local_network_access stay OFF (Grover's safe defaults)
# so an AI- or user-authored template can't read local files (file://) or reach
# internal/metadata services (SSRF) while rendering.
Grover.configure do |config|
  config.options = {
    format: "A4",
    cache: false,
    timeout: Integer(ENV.fetch("GROVER_TIMEOUT_MS", "30000"))
  }

  # The container ships a system Chromium and uses puppeteer-core, which needs the
  # binary path explicitly. Unset in local dev, where the feature degrades
  # gracefully (PdfGenerator raises a handled PdfGenerationError).
  executable_path = ENV["PUPPETEER_EXECUTABLE_PATH"].presence
  config.options[:executablePath] = executable_path if executable_path

  config.allow_file_uris = false
  config.allow_local_network_access = false
end
