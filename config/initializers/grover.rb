if defined?(Grover)
  Grover.configure { |c| c.options = { executable: ENV.fetch("CHROMIUM_PATH", nil), timeout: 30_000 } }
end
