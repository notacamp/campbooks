require "rails_helper"

RSpec.describe Ai::ChatService, type: :service do
  # Reasoning models (e.g. deepseek-v4-pro) intermittently emit a final-answer
  # JSON whose string values contain RAW control characters — a literal newline
  # or tab inside the "reply" text instead of the escaped \n / \t JSON requires.
  # Strict JSON.parse raises "invalid ASCII control character in string", which
  # used to discard the answer and leave Scout's typing indicator spinning.
  NL = "\n".freeze
  TAB = "\t".freeze

  describe ".parse_json_response" do
    it "recovers a reply with raw newline + tab inside the string (the prod bug)" do
      raw = %({"reply": "High-priority emails:#{NL}1.#{TAB}Jamie", "title": "Hi", "prompts": []})

      expect { JSON.parse(raw) }.to raise_error(JSON::ParserError) # precondition: strict parse fails

      result = described_class.parse_json_response(raw)
      expect(result["reply"]).to eq("High-priority emails:#{NL}1.#{TAB}Jamie")
      expect(result["title"]).to eq("Hi")
    end

    it "passes valid JSON straight through unchanged" do
      raw = %({"reply": "all good", "title": "t"})
      expect(described_class.parse_json_response(raw)).to eq("reply" => "all good", "title" => "t")
    end

    it "preserves braces and escaped quotes inside a string value alongside a raw newline" do
      raw = %({"reply": "use {curly} and say \\"hi\\"#{NL}done"})
      expect(described_class.parse_json_response(raw)["reply"]).to eq(%(use {curly} and say "hi"#{NL}done))
    end

    it "honors a custom object_start so global chat tool_calls still parse" do
      raw = %({"tool_call": "query_emails", "args": {"status": "fetched"}})
      result = described_class.parse_json_response(raw, object_start: /\{\s*"(reply|tool_call)"/)
      expect(result["tool_call"]).to eq("query_emails")
      expect(result["args"]).to eq("status" => "fetched")
    end

    it "extracts JSON wrapped in prose / a markdown fence even with raw control chars" do
      raw = "Here you go:#{NL}```json#{NL}{\"reply\": \"wrapped#{NL}answer\"}#{NL}```"
      expect(described_class.parse_json_response(raw)["reply"]).to eq("wrapped#{NL}answer")
    end

    it "never raises — falls back to a plain-text reply for non-JSON output" do
      result = nil
      expect { result = described_class.parse_json_response("I cannot help with that.") }.not_to raise_error
      expect(result["reply"]).to eq("I cannot help with that.")
      expect(result["suggested_actions"]).to eq([])
    end

    it "tolerates a nil / blank response" do
      expect(described_class.parse_json_response(nil)["reply"]).to eq("")
    end
  end

  describe ".repair_json_control_chars" do
    it "escapes control chars only inside string literals, leaving structural whitespace alone" do
      raw = %({#{NL}  "reply": "line1#{NL}line2"#{NL}})
      repaired = described_class.repair_json_control_chars(raw)

      # The newline inside the "reply" value became an escaped \n ...
      expect(repaired).to include('"line1\\nline2"')
      # ... while the pretty-print newlines between tokens are untouched (still raw).
      expect(repaired).to include("{#{NL}")
      expect { JSON.parse(repaired) }.not_to raise_error
    end

    it "does not double-escape sequences that were already escaped" do
      raw = %({"reply": "already\\nescaped"})
      expect(described_class.repair_json_control_chars(raw)).to eq(raw)
    end
  end
end
