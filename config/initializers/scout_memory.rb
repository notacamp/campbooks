# frozen_string_literal: true

# Scout's memory — the "Teach Scout something" input (Settings › Scout's memory)
# maps a free-text sentence to a bounded intent shape and executes it. Deterministic
# parsers (Scout::Memory::Parsers::*) cover the taught cases with no AI at all.
#
# When a parser does not match, the Teacher can fall back to a text model that maps
# the sentence onto ONE of those same bounded shapes (it never executes anything the
# validator does not recognise). That fallback is an injectable seam
# (Scout::Memory::Teacher.ai_mapper, a `String -> intent Hash | nil` callable):
#
#   * In development and test it is left nil, so the Teacher NEVER calls a real
#     provider — it simply replies "I can't learn that yet" for unrecognised
#     sentences. Specs inject a stub mapper to exercise the fallback path.
#   * A deployment that wants AI-assisted teaching assigns a mapper here (guarded by
#     Ai::ProviderSetup.available?). Set SCOUT_MEMORY_STUB_AI=1 to force it off.
#
# The bold layout is a preview feature; the AI mapper ships unwired by default.
Rails.application.config.after_initialize do
  Scout::Memory::Teacher.ai_mapper = nil if ENV["SCOUT_MEMORY_STUB_AI"] != "0"
end
