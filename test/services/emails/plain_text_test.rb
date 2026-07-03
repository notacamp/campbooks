# frozen_string_literal: true

require "test_helper"

module Emails
  class PlainTextTest < ActiveSupport::TestCase
    test "drops style/script content that a bare strip_tags would leak as text" do
      html = "<style>div.zm p.MsoNormal { margin: 0cm }</style>" \
             "<p>Please send the signed contract.</p><script>var x = 1</script>"

      text = PlainText.of(html)

      assert_equal "Please send the signed contract.", text
    end

    test "strips quoted reply history and its attribution from an HTML reply" do
      html = <<~HTML
        <div>Thanks — I will send it today.</div>
        <div>On Tue, Jul 1, 2026 at 9:00 AM Ana wrote:</div>
        <blockquote>Please send the signed contract back to us.</blockquote>
      HTML

      text = PlainText.of(html)

      assert_includes text, "I will send it today"
      refute_includes text, "signed contract"
      refute_includes text, "wrote:"
    end

    test "keeps quoted history when strip_quotes is false" do
      html = "<div>Top reply</div><blockquote>Original ask</blockquote>"

      assert_includes PlainText.of(html, strip_quotes: false), "Original ask"
    end

    test "plain-text bodies drop the >-quoted history" do
      raw = "I will handle it.\n> Please send the file\n> by Friday"

      assert_equal "I will handle it.", PlainText.of(raw)
    end

    test "blank input returns an empty string" do
      assert_equal "", PlainText.of(nil)
      assert_equal "", PlainText.of("   ")
    end
  end
end
