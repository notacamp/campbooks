require "rails_helper"

RSpec.describe Images::Dimensions do
  # Minimal, header-only fixtures — the reader never decodes pixel data, it only
  # parses the size fields, so a valid header is enough.
  def png(w, h)
    "\x89PNG\r\n\x1A\n".b + [ 13 ].pack("N") + "IHDR".b + [ w, h ].pack("N2") + ("\x00".b * 5)
  end

  def gif(w, h)
    "GIF89a".b + [ w, h ].pack("v2") + ("\x00".b * 8)
  end

  def jpeg(w, h)
    "\xFF\xD8".b + "\xFF\xC0".b + [ 17 ].pack("n") + "\x08".b + [ h, w ].pack("n2") + ("\x00".b * 20)
  end

  def bmp(w, h)
    "BM".b + ("\x00".b * 16) + [ w ].pack("l<") + [ h ].pack("l<") + ("\x00".b * 4)
  end

  def webp_vp8x(w, h)
    payload = ("\x00".b * 4) + [ w - 1 ].pack("V")[0, 3] + [ h - 1 ].pack("V")[0, 3]
    chunk = "VP8X".b + [ payload.bytesize ].pack("V") + payload
    "RIFF".b + [ ("WEBP".b + chunk).bytesize ].pack("V") + "WEBP".b + chunk
  end

  describe ".read" do
    it "reads PNG dimensions" do
      expect(described_class.read(png(640, 480))).to eq([ 640, 480 ])
    end

    it "reads GIF dimensions" do
      expect(described_class.read(gif(120, 90))).to eq([ 120, 90 ])
    end

    it "reads JPEG dimensions" do
      expect(described_class.read(jpeg(800, 600))).to eq([ 800, 600 ])
    end

    it "reads BMP dimensions (top-down/negative height normalised)" do
      expect(described_class.read(bmp(50, -50))).to eq([ 50, 50 ])
    end

    it "reads WEBP (VP8X) dimensions" do
      expect(described_class.read(webp_vp8x(300, 200))).to eq([ 300, 200 ])
    end

    it "detects a 1x1 tracking pixel" do
      expect(described_class.read(png(1, 1))).to eq([ 1, 1 ])
      expect(described_class.read(gif(1, 1))).to eq([ 1, 1 ])
    end

    it "reads the real-world 70-byte 1x1 PNG tracking pixel from production" do
      hex = "89504e470d0a1a0a0000000d49484452000000010000000108060000001f" \
            "15c4890000000b49444154789a6364f8cf500f0003860180" \
            "5a347d6b0000000049454e44ae426082"
      expect(described_class.read([ hex ].pack("H*"))).to eq([ 1, 1 ])
    end

    it "returns nil for non-image / unrecognised data" do
      expect(described_class.read("not an image at all, just text")).to be_nil
    end

    it "returns nil for nil or truncated input" do
      expect(described_class.read(nil)).to be_nil
      expect(described_class.read("\x89PNG".b)).to be_nil
    end
  end
end
